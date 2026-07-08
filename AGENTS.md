# AGENTS.md — TPT BioCAD

## What this repo is

Rust + Tauri desktop app for bioprinting/food-printing CAD and slicing, with an integrated rheology/CFD solver. Apache-2.0.

## Crate layout (dependency order)

```
core/          ← foundational types (serde, zip, thiserror)
geometry/      ← mesh/STL/lattice (depends on core)
fluid/         ← rheology solver: Carreau-Yasuda, Herschel-Bulkley, Bingham (depends on core)
slice/         ← toolpath + G-code generation (depends on core + geometry + fluid)
ui/            ← Tauri 2.0 shell (depends on all four)
```

`core` is the leaf. Never add a dependency from core → any other crate. `slice` is the convergence point where geometry and fluid meet.

## Build & verify commands

```bash
cargo build --workspace              # full build
cargo test --workspace               # run all tests
cargo fmt -- --check                 # format check (CI enforced)
cargo clippy --workspace -- -D warnings  # lint (CI enforced — any warning = failure)
```

CI runs all four on ubuntu. Windows and macOS jobs only run `cargo build` — formatting and lint issues may slip through on non-Linux.

## Single-crate testing

```bash
cargo test -p tpt-fluid              # test one crate
cargo test -p tpt-fluid -- --test solver_tests   # test one test module
```

## Frontend dev

```bash
npm install          # install vite + typescript
npm run dev          # starts Vite on :5173
```

Tauri's `beforeDevCommand` runs `npm run dev` automatically, so `cargo tauri dev` from `ui/` handles both. The Vite root is `ui/`, output goes to `dist/`.

## Key conventions

- All crates use `edition = "2021"` and `Apache-2.0` license.
- Error handling: each crate defines its own error type via `thiserror`. No panics in library code.
- The `fluid/` crate is the heaviest module (8 source files). Start there for rheology work.
- `ui/gen/` contains Tauri auto-generated bindings — do not edit manually.
- `spec.txt` is the authoritative design document — defines rheology models, material database, validation strategy, and roadmap.

## Validation strategy (from spec.txt)

The fluid solver is validated two ways:
1. Unit tests against closed-form analytical solutions (Newtonian/Poiseuille flow, Herschel-Bulkley plug flow)
2. Regression tests that fit model parameters against published rheometer data with explicit R² thresholds

Both are required deliverables, not afterthoughts.

## Gotchas

- `Cargo.lock` is gitignored (workspace convention — it's regenerated on build).
- The `.tpt` project file is a zip archive containing JSON members (`geometry.json`, `material.json`, `machine.json`, `profile.json`).
- opencascade-rs (B-Rep support) is planned but not yet integrated — see `geometry/Cargo.toml` comments.
- Klipper firmware plugin (M300-M303 commands) is in scope but not yet implemented.
