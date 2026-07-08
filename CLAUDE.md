# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this repo is

TPT BioCAD: a Rust + Tauri desktop app for bioprinting/food-printing CAD and slicing, with an integrated rheology/CFD solver. Unlike thermoplastic FDM slicers, it natively models shear-thinning and yield-stress fluids (bio-inks, chocolate, dough) to predict extrusion pressure and post-deposition slumping. Apache-2.0.

`spec.txt` is the authoritative design document — defines rheology models, material database, validation strategy, and roadmap. `TODO.md` tracks phase-by-phase progress against that roadmap; check it to see what's done vs. planned.

## Crate layout (dependency order)

```
core/          ← foundational types (serde, zip, thiserror) — Material, Machine, Profile, TptProject
geometry/      ← mesh/STL/lattice (depends on core)
fluid/         ← rheology solver: Carreau-Yasuda, Herschel-Bulkley, Bingham (depends on core)
slice/         ← toolpath + G-code generation (depends on core + geometry + fluid)
hal/           ← async hardware abstraction layer, serial comms with printers (depends on core, tokio, tokio-serial)
ui/            ← Tauri 2.0 shell (depends on core, geometry, fluid, slice)
```

`core` is the leaf — never add a dependency from core → any other crate. `slice` is the convergence point where geometry and fluid meet. `hal` is not wired into `ui`'s Tauri commands yet (see `ui/src/main.rs`); it exists as a standalone async serial layer for printer communication.

Outside the Cargo workspace: `klipper/tpt_biocad.py` is a Klipper firmware plugin (Python) that maps custom G-code commands M300–M303 (pneumatic pressure, thermal profiling, UV curing, coaxial cross-linker) onto Klipper primitives. Install instructions are in `klipper/README.md`.

## Build & verify commands

```bash
cargo build --workspace                   # full build
cargo test --workspace                    # run all tests
cargo fmt -- --check                      # format check (CI enforced)
cargo clippy --workspace -- -D warnings   # lint (CI enforced — any warning = failure)
```

CI (`.github/workflows/ci.yml`) runs all four on Ubuntu. Windows and macOS jobs only run `cargo build` — formatting and lint issues may slip through on non-Linux.

### Single-crate / single-test runs

```bash
cargo test -p tpt-fluid                              # test one crate
cargo test -p tpt-fluid -- --test solver_tests        # test one test module
```

Crate package names are prefixed (`tpt-core`, `tpt-geometry`, `tpt-fluid`, `tpt-slice`, `tpt-hal`, `tpt-ui`) even though directories are unprefixed (`core/`, `geometry/`, ...).

### Frontend / Tauri dev

```bash
npm install          # install vite + typescript
npm run dev           # starts Vite on :5173
```

Tauri's `beforeDevCommand` runs `npm run dev` automatically, so `cargo tauri dev` from `ui/` handles both. The Vite root is `ui/`, output goes to `dist/`. `ui/gen/` contains Tauri auto-generated bindings — do not edit manually.

## Architecture notes

- **UI ↔ Rust boundary**: `ui/src/main.rs` exposes plain functions as Tauri commands (`#[tauri::command]`) that call directly into `tpt_fluid` (e.g. `calculate_viscosity`, `calculate_pressure`, `calculate_slump`). There is no service/repository layer — commands call solver functions and map `Result<_, E>` to `Result<_, String>` for the frontend.
- **`.tpt` project file** (`core/src/project.rs`): a zip archive containing JSON members (`material.json`, `machine.json`, `profile.json`, plus geometry data). `TptProject::save`/`load` handle (de)serialization.
- **Rheology models** (`core/src/types.rs`): `RheologyModel` enum — `Newtonian`, `CarreauYasuda`, `HerschelBulkley`, `Bingham` — is the shared vocabulary between `fluid` (solving) and `ui` (display).
- **`fluid/` module split**: `models` (constitutive equations), `solver` (1D extrusion pressure/flow), `material` (database), `regression` (fits models to rheometer data), `safety` (shear stress / thermal interlocks), `slumping` (post-deposition shape prediction), `thermal` (tempering curves). This is the heaviest crate — start here for rheology work.

## Validation strategy (from spec.txt)

The fluid solver is validated two ways, both required deliverables:
1. Unit tests against closed-form analytical solutions (Newtonian/Poiseuille flow, Herschel-Bulkley plug flow) — see `fluid/src/validation.rs` (test-only module).
2. Regression tests that fit model parameters against published rheometer data with explicit R² acceptance thresholds — `fluid/src/regression.rs`.

## Key conventions

- All crates use `edition = "2021"` and `Apache-2.0` license.
- Error handling: each crate defines its own error type via `thiserror` (e.g. `HalError` in `hal/src/lib.rs`). No panics in library code.
- `Cargo.lock` is gitignored (workspace convention — it's regenerated on build).

## Gotchas

- opencascade-rs (B-Rep support) is planned but not yet integrated — see `geometry/Cargo.toml` comments and the Phase 1 spike item in `TODO.md`.
- `hal`'s pneumatic/screw extruder support and the Klipper M300–M303 plugin are implemented, but `hal` is not yet called from the Tauri UI layer.
- OpenCASCADE licensing (LGPL-2.1 + exception) compatibility with binary distribution is still an open item — don't assume it's cleared.
