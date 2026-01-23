# Repository Guidelines

## Project Structure & Module Organization
This repository is a Rust workspace. The root `Cargo.toml` defines shared dependencies and profiles. Key areas:
- `korangar/`: main client crate (`src/`, `components/`, `shaders/`, `archive/`, `client/`).
- `korangar-*`: feature crates (audio, interface, loaders, networking, video, etc.).
- `ragnarok-*`: standalone Ragnarok-related libraries (formats, packets, macros, bytes).
- `wiki/`: contributor, installation, and troubleshooting docs.

## Build, Test, and Development Commands
Run from the repo root:
- `cargo build --release`: build the workspace.
- `cargo run -p korangar --release --features debug`: run the client with dev tools.
- `cargo run -p korangar --release --features "debug unicode"`: enable Unicode TTY helpers.
- `cargo test`: run available tests (coverage is currently limited).
- `cargo fmt`: format all crates using `rustfmt.toml`.

Builds require the pinned nightly toolchain (`rust-toolchain.toml`). Shader compilation for `korangar` needs `slangc` (from Slang or the Vulkan SDK). Nix users can enter the dev shell via the repo flake.

## Coding Style & Naming Conventions
Use rustfmt defaults (4-space indentation, no tabs). Follow Rust naming conventions: `snake_case` for modules/functions, `CamelCase` for types/traits, and `SCREAMING_SNAKE_CASE` for constants. Keep public APIs focused and prefer explicit imports over globbing.

## Testing Guidelines
Tests are sparse; add `#[test]` modules near the code you touch or in `tests/` for integration coverage. Use `cargo test -p <crate>` to target a specific crate when iterating.

## Commit & Pull Request Guidelines
Commit messages in history are short, imperative sentences starting with a capital letter (e.g., "Update flake"). For PRs, include a concise summary, test results (or why tests were skipped), and screenshots or short clips for rendering/UI changes. Link related issues when available.

## Legal & Asset Notes
Korangar is a reverse-engineering project; do not include code or assets derived from GRAVITY IP. Runtime assets like `data.grf` and `rdata.grf` must stay local in `korangar/` and should never be committed.
