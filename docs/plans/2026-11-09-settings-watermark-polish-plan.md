# Settings and watermark polish: plan

Design: `docs/designs/2026-11-09-settings-watermark-polish-design.md`. Issue: #297.

1. **Logo limits (Rust).** Add `PP_LOGO_UPLOAD_MAX_SIZE` (default 20 MiB) to `UploadConfig`; `LogoNormaliser` takes the byte cap, uses a fixed 100 MP pixel cap and shrinks while loading; settings route derives its body limit from the config. Tests, README, FR-07.
2. **Diagonal repeat (image-worker).** Rotate the mark once, place copies on a staggered lattice rotated by -24°, composite in one call. Regression pixel test.
3. **Web polish.** Switch knob offset, French labels, static watermark sample tiles and loader change (cover lookup stays for the cover preview until #293), `SaveButton` lifecycle, Cancel ripple, no Saved toast, reduced motion. Tests.
4. **Verify.** Rust fmt, clippy, tests; `pnpm --dir web paraglide` and `verify`. Remove this plan before final merge approval.
