# Download a Photos selection

Approved scope for [#426](https://github.com/alekece/photo-proofing/issues/426#issuecomment-7082314847).

In the photographer's private Photos library, one checked photo downloads directly. For two or more, **Download** prepares one ZIP containing exactly the checked originals, including proofs and finals from different galleries. Show the existing busy indicator and start the download automatically once ready.

Reuse the Favorites archive preparation, size limits and filename collision handling. Validate the entire selection against the signed-in workspace. If the selection cannot be exported in full, show an error; do not deliver a partial ZIP or fall back to individual downloads. Leave Favorites and the client gallery unchanged.
