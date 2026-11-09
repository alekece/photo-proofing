# Settings and watermark polish: plan

Design: `docs/designs/2026-11-09-settings-watermark-polish-design.md`. Issue: #297.

1. **Logo limits (Rust).** `LogoNormaliser` gets a fixed 20 MiB `MAX_SIZE` constant and a fixed 100 MP pixel cap and shrinks while loading; the settings route derives its body limit from the constant. Tests, FR-07.
2. **Diagonal repeat (image-worker).** Rotate the mark once, place copies on a staggered lattice rotated by -24°, composite in one call. Regression pixel test.
3. **Web polish.** Switch knob offset, French labels, static watermark sample tiles and loader change (cover lookup stays for the cover preview until #293), `SaveButton` lifecycle, Cancel ripple, no Saved toast, reduced motion. Tests.
4. **Verify.** Rust fmt, clippy, tests; `pnpm --dir web paraglide` and `verify`. Remove this plan before final merge approval.
