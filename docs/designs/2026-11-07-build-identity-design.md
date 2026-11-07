# Frontend build identity

Issue: [#227](https://github.com/alekece/photo-proofing/issues/227).
The maintainer approved the short design proposed in conversation and requested
implementation and direct merge after verification.

Embed the deployment revision during the web build, using the same SHA as the
image tag. Show `Build <seven-character SHA>` at the bottom of photographer
Settings, with a control to copy the full SHA. Keep client pages unchanged.
Clipboard failure shows the full value for manual copying and an explicit error.
Local builds without a revision show Development, never an invented SHA.

Build-time embedding identifies the frontend loaded in the browser. A runtime
environment value or version endpoint could describe another deployment while
the browser still uses older assets; neither is needed. This identifies the
frontend only, not backend/worker versions. No dependencies or host details.
