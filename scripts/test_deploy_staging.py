import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("deploy_staging", ROOT / "scripts/deploy-staging.py")
deploy = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(deploy)


class ConfigurationTests(unittest.TestCase):
    def test_derives_origin_and_encodes_password(self):
        config = {
            "auth": {"app_origin": "https://proofs.test"},
            "session": {"secure_cookie": True},
        }
        env = deploy.runtime_environment(config, Path("/etc/photo-proofing/config.toml"),
                                         "192.168.1.76", "a" * 40, "a:@ /?#%")
        self.assertEqual(env["PP_APP_ORIGIN"], "https://proofs.test")
        self.assertEqual(env["PP_STAGING_HOST"], "proofs.test")
        self.assertEqual(env["PP_DATABASE_URL"], "postgres://pp:a%3A%40%20%2F%3F%23%25@postgres:5432/pp")
        self.assertEqual(env["PP_IMAGE_TAG"], "sha-" + "a" * 40)
        self.assertEqual(env["PP_BIND_ADDRESS"], "192.168.1.76")

    def test_rejects_invalid_origins_before_deployment(self):
        for origin in ("http://pp.test", "https://pp.test/path", "https://pp.test/",
                       "https://pp.test:8443", "https://user@pp.test", "https://pp.test?q=1",
                       "https://pp.test#x", "https://*.test", "https://pp.test\n"):
            with self.subTest(origin=origin), self.assertRaises(ValueError):
                deploy.runtime_environment(
                    {"auth": {"app_origin": origin}, "session": {"secure_cookie": True}},
                    Path("/config.toml"), "192.168.1.76", "a" * 40, "synthetic",
                )

    def test_rejects_missing_password_insecure_cookies_and_nonprivate_binding(self):
        valid = {"auth": {"app_origin": "https://pp.test"}, "session": {"secure_cookie": True}}
        for config, address, sha, password in (
            (valid, "192.168.1.76", "a" * 40, ""),
            ({"auth": {"app_origin": "https://pp.test"}}, "192.168.1.76", "a" * 40, "test"),
            (valid, "0.0.0.0", "a" * 40, "test"),
            (valid, "8.8.8.8", "a" * 40, "test"),
            (valid, "192.168.1.76", "staging", "test"),
        ):
            with self.subTest(address=address, sha=sha), self.assertRaises(ValueError):
                deploy.runtime_environment(config, Path("/config.toml"), address, sha, password)

    def test_rejects_malformed_toml_sections_without_tracebacks(self):
        for config in ({"auth": "invalid"}, {"auth": {"app_origin": "https://pp.test"},
                                            "session": "invalid"}):
            with self.subTest(config=config), self.assertRaises(ValueError):
                deploy.runtime_environment(config, Path("/config.toml"),
                                           "127.0.0.1", "a" * 40, "synthetic")

    def test_command_error_does_not_disclose_subprocess_output(self):
        with patch.object(subprocess, "run", return_value=subprocess.CompletedProcess(
            ["docker"], 1, stdout="secret-password", stderr="postgres://secret-password"
        )):
            with self.assertRaises(RuntimeError) as error:
                deploy.run(["docker", "compose", "pull"], {})
        self.assertNotIn("secret-password", str(error.exception))
        self.assertIn("failed", str(error.exception))

    def test_failed_pull_never_starts_containers(self):
        commands = []

        def fake_run(command, env):
            commands.append(command)
            if command[-1] == "pull":
                raise RuntimeError("pull failed")
            return ""

        with patch.object(deploy, "run", side_effect=fake_run):
            with self.assertRaises(RuntimeError):
                deploy.start({})
        self.assertFalse(any("up" in command for command in commands))


class ComposeContractTests(unittest.TestCase):
    def test_merged_staging_contract(self):
        with tempfile.TemporaryDirectory() as directory:
            config = Path(directory) / "config.toml"
            config.write_text('[auth]\napp_origin = "https://pp.test"\n')
            env = {
                **os.environ,
                "PP_RUNTIME_CONFIG": str(config),
                "PP_APP_ORIGIN": "https://pp.test",
                "PP_STAGING_HOST": "pp.test",
                "PP_BIND_ADDRESS": "127.0.0.1",
                "PP_IMAGE_TAG": "sha-" + "a" * 40,
                "POSTGRES_PASSWORD": "synthetic-only",
                "PP_DATABASE_URL": "postgres://pp:synthetic-only@postgres:5432/pp",
            }
            result = subprocess.run(
                ["docker", "compose", "--env-file", "/dev/null", "-p", "photo-proofing-staging",
                 "-f", str(ROOT / "docker-compose.yml"), "-f", str(ROOT / "compose.staging.yml"),
                 "config", "--format", "json"],
                env=env, capture_output=True, text=True, check=True,
            )
        services = json.loads(result.stdout)["services"]
        for name, image in (("api", "pp-api"), ("worker", "pp-image-worker"), ("web", "pp-web")):
            self.assertEqual(services[name]["image"], f"ghcr.io/alekece/{image}:sha-" + "a" * 40)
            self.assertNotIn("build", services[name])
            self.assertFalse(services[name].get("ports"))
        self.assertFalse(services["postgres"].get("ports"))
        self.assertEqual(services["api"]["environment"]["PP_CONFIG"], "/etc/pp/config.toml")
        self.assertNotIn("PP_SESSION_COOKIE_SECURE", services["api"]["environment"])
        self.assertNotIn("PP_APP_ORIGIN", services["api"]["environment"])
        self.assertEqual(services["web"]["environment"]["ORIGIN"], "https://pp.test")
        self.assertEqual(services["web"]["environment"]["PP_API_INTERNAL_URL"], "http://api:8080")
        self.assertEqual({p["target"] for p in services["proxy"]["ports"]}, {443})
        self.assertTrue(all(p["host_ip"] == "127.0.0.1" for p in services["proxy"]["ports"]))
        for service in services.values():
            self.assertEqual(service["restart"], "unless-stopped")
            self.assertEqual(service["logging"]["options"]["max-size"], "10m")
        self.assertTrue(any(v["target"] == "/data" for v in services["proxy"]["volumes"]))
        self.assertTrue(any(v["target"] == "/var/lib/pp/photos" for v in services["api"]["volumes"]))


if __name__ == "__main__":
    unittest.main()
