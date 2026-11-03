#!/usr/bin/env python3
"""Deploy one published staging release; run under Doppler, never print secrets."""

import argparse
import ipaddress
import os
from pathlib import Path
import re
import subprocess
import sys
import tomllib
from urllib.parse import quote, urlsplit


ROOT = Path(__file__).resolve().parents[1]


def runtime_environment(config, config_path, address, sha, password):
    if not all(isinstance(config.get(section, {}), dict) for section in ("auth", "session")):
        raise ValueError("auth and session must be TOML tables")
    origin = config.get("auth", {}).get("app_origin", "")
    if not isinstance(origin, str) or not re.fullmatch(r"https://[a-z0-9.-]+", origin):
        raise ValueError("auth.app_origin must be an HTTPS hostname without a port or path")
    host = urlsplit(origin).hostname
    if not host or len(host) > 253 or not all(
        re.fullmatch(r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?", label)
        for label in host.split(".")
    ):
        raise ValueError("auth.app_origin contains an invalid hostname")
    if config.get("session", {}).get("secure_cookie") is not True:
        raise ValueError("session.secure_cookie must be true")
    binding = ipaddress.IPv4Address(address)
    if not (binding.is_loopback or any(binding in network for network in (
        ipaddress.IPv4Network("10.0.0.0/8"),
        ipaddress.IPv4Network("172.16.0.0/12"),
        ipaddress.IPv4Network("192.168.0.0/16"),
    ))):
        raise ValueError("bind address must be loopback or an RFC1918 LAN address")
    if not re.fullmatch(r"[0-9a-f]{40}", sha):
        raise ValueError("staging must resolve to a full commit SHA")
    if not password:
        raise ValueError("POSTGRES_PASSWORD is required; run through Doppler")
    return {
        **os.environ,
        "PP_RUNTIME_CONFIG": str(config_path.resolve()),
        "PP_APP_ORIGIN": origin,
        "PP_STAGING_HOST": host,
        "PP_BIND_ADDRESS": str(binding),
        "PP_IMAGE_TAG": "sha-" + sha,
        "POSTGRES_PASSWORD": password,
        "PP_DATABASE_URL": f"postgres://pp:{quote(password, safe='')}@postgres:5432/pp",
    }


def run(command, env):
    result = subprocess.run(command, env=env, capture_output=True, text=True)
    if result.returncode:
        # Docker diagnostics can contain expanded credentials or connection URLs.
        raise RuntimeError(
            f"{' '.join(command)} failed (exit {result.returncode}); "
            "output withheld because it may contain secrets"
        )
    return result.stdout.strip()


def start(env):
    compose = [
        "docker", "compose", "--env-file", "/dev/null",
        "--project-name", "photo-proofing-staging",
        "-f", str(ROOT / "docker-compose.yml"),
        "-f", str(ROOT / "compose.staging.yml"),
    ]
    run([*compose, "config", "--quiet"], env)
    run([*compose, "pull"], env)
    run([*compose, "up", "-d", "--no-build", "--wait", "--wait-timeout", "180"], env)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=Path("/etc/photo-proofing/config.toml"))
    parser.add_argument("--bind-address", default="127.0.0.1",
                        help="Default is loopback for owner-controlled account bootstrap")
    args = parser.parse_args()
    try:
        with args.config.open("rb") as source:
            config = tomllib.load(source)
        password = os.environ.get("POSTGRES_PASSWORD", "")
        env = runtime_environment(config, args.config, args.bind_address, "0" * 40, password)
        sha = run([
            "gh", "api", "repos/alekece/photo-proofing/commits/staging", "--jq", ".sha",
        ], os.environ)
        env = runtime_environment(config, args.config, args.bind_address, sha, password)
        print(f"Deploying staging commit {sha} at {env['PP_APP_ORIGIN']} "
              f"(binding {args.bind_address})", flush=True)
        start(env)
    except tomllib.TOMLDecodeError:
        print("Deployment failed: invalid TOML configuration (contents withheld)", file=sys.stderr)
        return 1
    except (OSError, ValueError, RuntimeError) as error:
        print(f"Deployment failed: {error}", file=sys.stderr)
        return 1
    print(f"Staging containers ready for commit {sha}; browser DNS/trust require separate checks.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
