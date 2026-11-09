# Dockhand staging deployment plan

Design: [Dockhand staging deployment](../designs/2026-11-09-dockhand-staging-deployment-design.md);
issue [#288](https://github.com/alekece/photo-proofing/issues/288).

1. CD: after the `staging` Git tag advances, point `:staging` on the three images at the
   `sha-` image, only when the tag equals the candidate commit. (`.github/workflows/cd.yml`)
2. Add `deploy/compose.yml`: single-file stack, `:staging` images, no values or
   secrets, inline Caddyfile, external volumes.
3. CI: validate the stack with synthetic values. (`.github/workflows/ci.yml`)
4. README: document Dockhand setup; mark Ansible as the working path until cutover.
5. Operator (needs the user): auto-update trial, create the stack in Dockhand, stop
   `pp-staging-deploy.timer`, deploy, verify, then retire `deploy/staging.yml`, `deploy/verify.yml`,
   its CI steps, `compose.staging.yml`, `docker/Caddyfile.staging` and the Ansible docs.
6. Follow-up issue: Uptime Kuma check comparing the running SHA with the `staging` tag.
