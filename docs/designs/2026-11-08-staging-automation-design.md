# Versioned staging automation

Issue: [#236](https://github.com/alekece/photo-proofing/issues/236).
Related: [#135](https://github.com/alekece/photo-proofing/issues/135) (image publication; its
[design](2026-11-03-preproduction-deployment-design.md) left the server-side consumer out of scope).

Status: draft, awaiting maintainer approval.

## Intent and scope

The staging server already redeploys every published release: a systemd timer runs a
reconcile playbook every minute, which resolves the `staging` tag, checks out that
commit, runs `deploy/staging.yml` and verifies the API over trusted HTTPS. The timer
units and the reconcile playbook live only on the server, so changes to them are
untracked and unreviewed. Two gaps follow from running it for a few days:

- Old release images are never removed (about 1 GB per merge).
- A failed attempt reports only a coarse phase, so a transient failure cannot be told
  apart from a real one.

This design versions the generic automation in the repository and closes both gaps.
It changes no application code and no production behavior. Real hostnames, addresses,
volume names, account details and certificates stay on the server; the repository and
its checks use synthetic values only.

## Approach

1. **Version the automation.** Add the existing files at the same paths:
   `deploy/automatic.yml`, `deploy/install-timer.yml` and
   `deploy/templates/pp-staging-deploy.{service,timer}.j2`. They contain no private
   values: the server supplies them through `/etc/pp/automatic.yml` and
   `/etc/pp/staging.yml`, which stay outside Git.
2. **Prune after a verified deployment.** `deploy/staging.yml` is release-owned, so the
   cleanup ships with each release and needs no server change. After reconciliation it
   keeps the running release plus the five newest `sha-` tags of each application image
   (`pp_image_keep`, default 5) and removes older ones. Removal never forces, so an
   image still used by a container is kept. A cleanup failure is reported as a safe
   note and never fails an otherwise good deployment. Other images, volumes and
   containers are untouched.
3. **Name the failing step.** `deploy/automatic.yml` captures the inner playbook's
   output privately and, on failure, adds only the last started task name to the safe
   message, for example `failed during release deployment (step: Pull the complete
   release before reconciliation)`. Task names are static text written in the repository;
   the name is only reported if it matches a plain-text allowlist pattern. Task output,
   variables and paths stay suppressed.

## Update model

`deploy/staging.yml` and the prune step are replaced on every deployment because the
reconcile playbook checks out the release before running it. The reconcile playbook and
the unit templates are the small trusted bootstrap that performs that checkout, so they
keep running from a server-installed copy and are refreshed deliberately by the
operator through `deploy/install-timer.yml`, not silently from a moving tag. The one-time
refresh for this change is needed for the failing-step message only; pruning works as
soon as the release deploys.

## Safeguards preserved

Serialised runs under the lock, refusal to touch operator changes in the release
checkout, explicit server-owned settings, `no_log` on credential-bearing tasks, trusted
HTTPS verification, an atomic success record, and no creation of data volumes.

## Verification

- `deploy/verify.yml` gains synthetic checks: the playbooks pass `--syntax-check`; the
  reconcile playbook still rejects missing settings before any network or Docker access;
  the rendered units contain only values supplied by the test.
- The prune selection (which tags are kept or removed) is tested against a synthetic
  image list, including a release newer than the retention window and an image in use.
- The failing-step extraction is tested with synthetic output, including output that
  contains a secret-looking value outside a task name, which must not appear.
- Existing CI checks for `deploy/` continue to run. Server-side behavior (timer firing,
  actual removal) is confirmed by the maintainer on staging after the one-time refresh;
  this work does not claim it.

## Documentation

Update the README's LAN staging section: the timer-based automation now exists, how the
operator installs or refreshes it, and the retention setting. Remove the sentence saying
automation is not configured. Add no private details.

## Accepted limitations

No zero-downtime deployment or database rollback; a worker job running during a release
may be interrupted and retried. Rollback still means moving the `staging` tag, which CD
deliberately prevents; a documented manual override is out of scope here.
