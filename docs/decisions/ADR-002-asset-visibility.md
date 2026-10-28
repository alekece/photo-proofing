# ADR-002: Separate source, proof, and final assets

## Status

Accepted for MVP.

## Decision

The system stores separate private asset roles:

- Original JPEG and RAW sources for photographer access only.
- Restricted proof previews for client selection.
- Optional watermarked versions of proofs.
- Published retouched final delivery files.
- Temporary ZIP exports based on immutable manifests.

The API, not the frontend, verifies both gallery state and asset role before issuing a short-lived access grant.

## Consequences

Clients cannot download from Proofing, Selection received, or Retouching. In Delivery, clients see only final assets deliberately published in the delivery manifest. This requires additional storage and worker processing, but prevents accidental proof or source exposure.