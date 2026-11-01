# ACA Film Studio — ALEJANDRO Creative Academy derivative

Local source fork of [FilmCraft](https://github.com/storytold/filmcraft) at `7e17c704662c951b5c267eef688207f2f936e267`.
This is an independent ACA derivative, not an official ArtCraft distribution or endorsement.

Display branding and the actual default egui palette use purple surfaces and gold controls.
Native document formats, engine/command IDs, extensions, semantic meters/errors, and saved documents remain upstream-compatible.
MIT/Apache license texts, NOTICE, contributor attribution and dependency/asset notices remain.
The upstream ArtCraft trademark artwork was excluded; ACA branding is original code-drawn type/geometry.

Browser build: cargo xtask web.
Use Rust's wasm32-unknown-unknown target and wasm-bindgen CLI exactly 0.2.129.
Source edits are implemented; successful browser compilation and interactive behavior must be recorded separately.
Do not publish the upstream installers as rebranded Academy binaries or claim upstream features are tested by this palette change.

## Academy build pipeline

The fork-only `Academy WASM build` workflow is manual (`workflow_dispatch`) and uses two sequential jobs on standard public Ubuntu runners. It builds this modified source with Rust1.95 and wasm-bindgen0.2.129; it performs no signing, deployment, or desktop release matrix and reads no user secrets. Browser verification and upstream full-suite CI remain separate gates.

Source: https://github.com/mypurpletimemachine2023/alejandro-film-studio

## Browser recovery follow-up

The Academy edition keeps validated clean native saves as well as dirty recovery checkpoints,
restores the newest valid record with previous-valid/legacy fallbacks, and exposes explicit
checkpointProject/flushRecovery acknowledgements. Browser downloads are requests, not proof of a
file accepted on disk. Desktop-only Project Manager collection is disabled in WASM. See
docs/academy-browser-recovery.md for scope, limits, media ownership and the focused regression tests.
