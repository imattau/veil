# Repository Guidelines

## Project Structure & Module Organization
VEIL is a Rust workspace organized by protocol layer under `crates/`.

- `crates/veil-core`: shared primitives (`types`, hashing, tag derivation, errors)
- `crates/veil-codec`: `ObjectV1`/`ShardV1` encoding and schema handling
- `crates/veil-crypto`: AEAD and signing traits/helpers
- `crates/veil-fec`: FEC profiles and sharding entry points
- `crates/veil-node`: node cache, forwarding, and runtime state
- `crates/veil-transport`: transport lane abstraction
- `crates/veil-sim`: simulation scaffolding
- `examples/end_to_end.rs`: runnable example placeholder

Specs and planning docs live at the root: `SPEC.md`, `ROADMAP.md`, `README.md`.

## Build, Test, and Development Commands
Run all commands from the repository root.

- `cargo check --workspace` — fast compile validation across all crates
- `cargo test --workspace` — run unit/integration/doc tests for the workspace
- `cargo fmt --all` — format code consistently
- `cargo clippy --workspace --all-targets -- -D warnings` — lint and fail on warnings
- `cargo run --example end_to_end` — run the example binary

## Coding Style & Naming Conventions
- Follow standard Rust formatting (`rustfmt`) with 4-space indentation.
- Use `snake_case` for modules/functions/files, `CamelCase` for types/traits, `SCREAMING_SNAKE_CASE` for constants.
- Keep crate boundaries clean: put shared protocol types in `veil-core`, and avoid circular dependencies between crates.
- Prefer small, explicit public APIs (`pub`) and keep internals private by default.

## Testing Guidelines
- Add tests near the code (`#[cfg(test)]`) for unit behavior; add integration tests under `crates/<crate>/tests/` when cross-module behavior matters.
- Name tests for behavior, e.g. `derives_feed_tag_deterministically`.
- Cover protocol-critical paths (tag derivation, object/shard encoding, dedupe/cache decisions, forwarding rules).
- There is no fixed coverage gate yet; new logic should ship with meaningful tests.

## Commit & Pull Request Guidelines
- Current history is minimal (`Initial commit`), so keep commit subjects short, imperative, and descriptive (<= 72 chars), e.g. `Add deterministic shard ID helper`.
- Keep commits focused (one logical change each).
- PRs should include: purpose, key design decisions, test evidence (`cargo test --workspace` output), and any spec impact (`SPEC.md` references).
