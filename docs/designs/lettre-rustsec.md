# Patch lettre SMTP vulnerabilities

[Issue #396](https://github.com/alekece/photo-proofing/issues/396)

Require lettre 0.11.24 and resolve that release in the workspace lockfile to address RUSTSEC-2026-0560 through RUSTSEC-2026-0565. Preserve the existing mail features and dependency audit policy.

Run the workspace Rust checks and the dependency audit. Target main so the stacked PRs can pick up the fix through their base branches.
