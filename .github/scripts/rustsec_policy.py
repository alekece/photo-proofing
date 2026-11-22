import copy
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib


ADVISORY_ID = re.compile(r"\bRUSTSEC-[0-9]{4}-[0-9]{4}\b", re.ASCII)


def read_findings(records, exit_code):
    findings = set()
    summary = None
    errors = 0

    for record in records:
        kind = record["type"]
        fields = record.get("fields") or {}
        if kind == "log" and fields.get("level") == "ERROR":
            raise ValueError("cargo-deny reported a tool error")

        if kind == "summary":
            if summary is not None:
                raise ValueError("cargo-deny emitted multiple summaries")
            summary = fields.get("advisories") or {}

        if kind != "diagnostic":
            continue

        if fields.get("code") == "index-failure":
            raise ValueError("cargo-deny could not check the registry index")

        advisory = fields.get("advisory")
        if fields.get("severity") == "error":
            if not advisory:
                raise ValueError("cargo-deny reported a non-advisory error")
            errors += 1

        if advisory:
            id = advisory.get("id")
            if not isinstance(id, str) or not ADVISORY_ID.fullmatch(id):
                raise ValueError("Invalid RustSec ID")
            findings.add(id)

    if summary is None or type(summary.get("errors")) is not int:
        raise ValueError("Missing cargo-deny advisory summary")

    if summary["errors"] != errors or exit_code != int(errors > 0):
        raise ValueError("Advisory summary, diagnostics and exit code disagree")

    return sorted(findings)


def untracked_advisories(ids, open_items):
    tracked = {
        id
        for item in open_items
        if any(label["name"] == "rustsec" for label in item["labels"])
        for id in ADVISORY_ID.findall(item["title"])
    }
    return [id for id in ids if id not in tracked]


def probe(features=(), config=None, fetch=False):
    result = subprocess.run(
        ["cargo", "deny", "--locked", "--workspace", *features, "--format", "json", "check",
         *([] if config is None else ["--config", str(config)]), "advisories",
         *([] if fetch else ["--disable-fetch"])],
        capture_output=True,
        text=True,
        check=False,
    )
    records = [json.loads(line) for output in (result.stdout, result.stderr)
               for line in output.splitlines() if line.strip()]
    return records, result.returncode


def classify_ignore(id, default_probe, all_features_probe):
    findings = read_findings(*default_probe) + read_findings(*all_features_probe)
    return "affected" if id in findings else "obsolete"


def upgrade_targets(id, records):
    return {
        (node["Krate"]["name"], node["Krate"]["version"])
        for record in records
        if record["type"] == "diagnostic" and (record["fields"].get("advisory") or {}).get("id") == id
        for root in record["fields"].get("graphs", [])
        for node in [root, *root.get("parents", [])]
    }


def ignore_probes(source):
    policy = tomllib.loads(source)
    lines = source.splitlines(keepends=True)
    seen = set()
    for index, entry in enumerate(policy.get("advisories", {}).get("ignore", [])):
        id = entry["id"] if isinstance(entry, dict) else entry
        if not ADVISORY_ID.fullmatch(id) or id in seen:
            raise ValueError(f"Invalid or duplicate RustSec ignore: {id}")

        expected = copy.deepcopy(policy)
        del expected["advisories"]["ignore"][index]
        # Compare parsed policy so a similar ID or unsupported layout cannot
        # cause a different setting to be changed.
        matches = []
        for position, line in enumerate(lines):
            if f'"{id}"' not in line and f"'{id}'" not in line:
                continue
            candidate = "".join(lines[:position] + lines[position + 1:])
            try:
                if tomllib.loads(candidate) == expected:
                    matches.append(candidate)
            except tomllib.TOMLDecodeError:
                continue
        if len(matches) != 1:
            raise ValueError(f"Cannot remove exactly {id} from the temporary policy")

        seen.add(id)
        yield id, matches[0]


def review_ignores():
    findings = {}
    targets = {}
    with tempfile.TemporaryDirectory(prefix="rustsec-probes-", dir=os.environ.get("RUNNER_TEMP")) as directory:
        for id, source in ignore_probes(Path("deny.toml").read_text()):
            config = Path(directory) / f"{id}.toml"
            config.write_text(source)
            default = probe(config=config)
            all_features = probe(["--all-features"], config)
            findings[id] = classify_ignore(id, default, all_features)
            if findings[id] == "affected":
                targets[id] = sorted(upgrade_targets(id, default[0] + all_features[0]))

    return {"findings": findings, "upgrade_targets": targets}


def removed_ignores(before, after, candidates, dependencies_changed):
    remaining = after["advisories"]["ignore"]
    expected = copy.deepcopy(before)
    original = expected["advisories"]["ignore"]
    removed = [entry for entry in original if entry not in remaining]
    original[:] = [entry for entry in original if entry not in removed]
    if not removed or expected != after:
        raise ValueError("Patch must remove ignores without changes outside those entries")

    ids = [entry["id"] if isinstance(entry, dict) else entry for entry in removed]
    for advisory in ids:
        if candidates.get(advisory) not in {"affected", "obsolete"}:
            raise ValueError(f"{advisory} is not a scan candidate")

    if dependencies_changed != any(candidates[advisory] == "affected" for advisory in ids):
        raise ValueError("A dependency change is required only for affected ignores")

    return ids


def verify_policy(base, report):
    # Check Git's actual changes, including mode and path, before reading any
    # patched manifests or running cargo metadata.
    raw = subprocess.check_output(["git", "diff", "--raw", "--no-renames", "-z", base, "HEAD"])
    fields = raw.decode().split("\0")
    changes = set()
    for metadata, path in zip(fields[::2], fields[1::2]):
        old_mode, new_mode, _, _, status = metadata.split()
        if (old_mode, new_mode, status) != (":100644", "100644", "M") or path not in {"deny.toml", "Cargo.lock"}:
            raise ValueError("Patch must only modify existing deny.toml and Cargo.lock files")
        changes.add(path)

    if "deny.toml" not in changes:
        raise ValueError("Patch must remove at least one ignore")

    before = tomllib.loads(subprocess.check_output(["git", "show", f"{base}:deny.toml"], text=True))
    after = tomllib.loads(Path("deny.toml").read_text())
    candidates = {advisory: report["findings"][advisory] for advisory in report["candidate_ids"]}
    return removed_ignores(before, after, candidates, bool(changes - {"deny.toml"}))


def focused_updates(before, after, targets):
    registry = "registry+https://github.com/rust-lang/crates.io-index"

    def versions(lock):
        return {(package["name"], package["version"]) for package in lock["package"]
                if package.get("source") == registry and re.fullmatch(r"[a-zA-Z0-9_-]+", package["name"])
                and re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", package["version"])}

    original, proposed = versions(before), versions(after)
    return sorted(
        (name, old, new)
        for name, old in (original - proposed) & targets
        for new_name, new in proposed - original
        if name == new_name and tuple(map(int, new.split("."))) > tuple(map(int, old.split(".")))
    )


def verify_upgrade(base, targets):
    original = tomllib.loads(subprocess.check_output(["git", "show", f"{base}:Cargo.lock"], text=True))
    proposed = tomllib.loads(Path("Cargo.lock").read_text())
    updates = focused_updates(original, proposed, targets)
    if not updates:
        raise ValueError("Expected an upgrade of the affected crate or one of its direct parents")

    with tempfile.TemporaryDirectory(prefix="rustsec-base-", dir=os.environ.get("RUNNER_TEMP")) as directory:
        subprocess.run(["git", "worktree", "add", "--detach", directory, base], check=True, stdout=sys.stderr)
        try:
            lockfile = Path(directory) / "Cargo.lock"
            original_text = lockfile.read_text()
            for name, old, new in updates:
                lockfile.write_text(original_text)
                result = subprocess.run(
                    ["cargo", "update", "-p", f"{name}@{old}", "--precise", new],
                    cwd=directory, stdout=sys.stderr, check=False,
                )
                if result.returncode == 0 and tomllib.loads(lockfile.read_text()) == proposed:
                    return
        finally:
            subprocess.run(["git", "worktree", "remove", "--force", directory], check=True, stdout=sys.stderr)

    raise ValueError("The proposed lockfile could not be reproduced with a targeted cargo update")


def verify_proposal(base, report_path, patch_path):
    report = json.loads(Path(report_path).read_text())
    head = subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip()
    if report["sha"] != base or head != base:
        raise ValueError("Verifier checkout does not match the scan")

    subprocess.run(["cargo", "deny", "fetch", "db"], check=True, stdout=sys.stderr)
    modes = ((), ("--all-features",))
    baseline = [set(read_findings(*probe(mode))) for mode in modes]
    subprocess.run(
        ["git", "-c", "core.hooksPath=/dev/null", "-c", "user.name=RustSec validation",
         "-c", "user.email=rustsec@example.invalid", "am", "--no-3way", patch_path],
        check=True,
        stdout=sys.stderr,
    )
    parent = subprocess.check_output(["git", "rev-parse", "HEAD^"], text=True).strip()
    if parent != base:
        raise ValueError("Expected exactly one commit on the scanned revision")

    removed = verify_policy(base, report)
    affected = [id for id in removed if report["findings"][id] == "affected"]
    if affected:
        targets = set.intersection(*(set(map(tuple, report["upgrade_targets"][id])) for id in affected))
        verify_upgrade(base, targets)

    for mode, before in zip(modes, baseline):
        after = set(read_findings(*probe(mode)))
        if after.intersection(removed) or after - before:
            raise ValueError("The proposed policy reveals a removed advisory or a new finding")

    subprocess.run(
        ["cargo", "deny", "--locked", "--workspace", "check", "bans", "licenses", "sources"],
        check=True,
        stdout=sys.stderr,
    )
    if affected:
        subprocess.run(["cargo", "check", "--locked", "--workspace", "--all-targets"], check=True, stdout=sys.stderr)

    return removed


if __name__ == "__main__":
    command, *args = sys.argv[1:]
    if command == "daily":
        result = read_findings(*probe(fetch=True))
    elif command == "weekly":
        result = review_ignores()
    elif command == "untracked":
        result = untracked_advisories(*json.load(sys.stdin))
    elif command == "verify":
        result = verify_proposal(*args)
    else:
        raise ValueError(f"Unknown RustSec command: {command}")

    print(json.dumps(result))
