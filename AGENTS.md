# icy — Agent Guidelines

`icy` is an agent-first terminal workspace and IDE built in Rust with a native JSNIR (TypeScript/TSX over QuickJS + wgpu) UI.

## Engineering Principles

1. **Separate state from runtime.** Keep pure workspace/session/link logic in `crates/icy-core` testable without a window, GPU, or running UI.
2. **Isolate platform code.** OS-specific logic (`#[cfg(unix)]` / `#[cfg(windows)]`) belongs strictly inside `crates/icy-core/src/platform/`. Core modules and UI code must stay platform-agnostic.
3. **Keep the VT backend swappable.** Talk to terminal emulation through `icy_core::vt::VtEngine` (`AlacrittyVt` today; `libghostty-vt` can be plugged in later without touching `session.rs` or `icy-app`).
4. **Follow the Design System strictly.** Every color, spacing, radius, font, shadow, and motion value in `app/` must come from `app/theme.ts` and `DESIGN.md` (Accordee system). Never introduce ad-hoc hex colors or magic layout numbers in components.
5. **Windows builds from day one.** Every change must compile cleanly and pass tests/clippy on both `ubuntu-24.04` and `windows-2022` in GitHub Actions CI.
6. **Keep JSNIR unmodified.** Do not edit `../Jurui` from `icy` feature work; pin the engine commit in `jsnir-ref.txt` and document engine gaps in `ARCHITECTURE.md`.
7. **Language conventions.** Code, identifiers, tests, and code comments are in English. User-facing project docs (`README.md`, `ARCHITECTURE.md`, `DESIGN.md`) are in Spanish.
