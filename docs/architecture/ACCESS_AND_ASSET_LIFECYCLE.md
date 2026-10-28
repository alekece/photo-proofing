# Access and Asset Lifecycle

## Security invariants

1. Source assets are never deliverable through a client gallery link.
2. The backend checks authorisation, gallery state, expiry, revocation, and asset role before issuing every preview or download URL.
3. A hidden button, obscured path, browser route, `Referer` header, or CSS watermark is never treated as access control.
4. A client in Delivery may access only published final asset IDs in the current delivery manifest.
5. Expiry and revocation block new access server-side. They cannot recall files already copied by a client.

## Asset classes

| Asset | Owner access | Client access |
| --- | --- | --- |
| Original JPEG or RAW source | Authenticated photographer only | Never |
| Proof preview | Photographer | Only in Proofing, Selection received, and Retouching |
| Watermarked proof preview | Photographer | Only where watermarking is enabled, in proofing states |
| Final delivery file | Photographer | Only in Delivery if published in delivery manifest |
| Selected source ZIP | Photographer only | Never |
| Delivery ZIP | Photographer | Only in Delivery if photographer enabled it |

Originals, proofs, finals, and ZIPs must live under distinct private namespaces. Object keys, source filenames, source IDs, and storage paths are never exposed to the client.

## Client access protocol

1. The client opens an opaque, high-entropy share link.
2. The API validates its stored token hash, expiry, revocation state, optional passcode, gallery state, and rate limit.
3. After validation, the API creates a short-lived client session using secure cookie settings.
4. Every proof and final asset request rechecks the session and the state-specific asset rule.
5. The API issues a short-lived, path-scoped access grant only for the allowed asset. The grant must not permit directory listing or another gallery's assets.

Raw tokens, passcodes, signed URLs, and client asset URLs must not be logged, placed in analytics, or leaked in referrer headers. Rate-limit token and passcode attempts. Use CSRF protection for authenticated browser write actions.

## Lifecycle transitions

### Draft to Proofing

The photographer explicitly enables a share link. Client access begins only after the link has passed all validation.

### Proofing to Selection received

This reflects that the photographer has observed enough client selection activity to review it. Favourites remain auto-saved. This state does not make files downloadable.

### Selection received to Retouching

The photographer creates a `SelectionSnapshot`. That frozen list, including its timestamp and source file identifiers, is the exact input for a photographer-only asynchronous ZIP export. Subsequent changes to live favourites do not change an already requested export.

### Retouching to Delivery

The photographer explicitly publishes final files into a `DeliveryManifest`. This transition removes proof and unselected-image visibility for clients at the API level. It does not merely hide them in the frontend.

### Any state to Expired

The API refuses new client sessions and asset grants. Cache-control policy and short grant lifetime must make expiry and revocation predictable. The photographer continues to have authenticated access until retention or deletion rules apply.

## Watermarking

Watermarking is optional. When enabled, it is generated into the proof asset by the worker. A watermark change creates a new proof version rather than altering an original. Watermarks discourage casual reuse but are not DRM and cannot prevent screenshots.

## Retention and deletion

Each gallery has an access expiry date and a separate photographer-data retention date. They are not the same concept.

Before automatic deletion is released, test the removal of originals, RAW sources, proofs, finals, thumbnails, delivery ZIPs, selected-source ZIPs, caches, and database metadata. Backups require their own documented retention and restore process. Do not claim immediate or irreversible deletion until this is verified.