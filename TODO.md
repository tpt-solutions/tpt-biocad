# TPT BioCAD — Task Checklist

Tracks work derived from `spec.txt` (Section 8 Roadmap, plus items added during the spec review on 2026-07-01). Check items off as completed.

## Phase 0: Spec & Groundwork
- [x] Draft initial design document (`spec.txt`)
- [x] Review spec for risks, add Risks & Assumptions section
- [x] Define rheology solver validation strategy (no-hardware plan)
- [x] Decide firmware target (Klipper first) and document approach
- [x] Clarify `.tpt` file format as a zip archive

## Phase 1: Foundation (Months 1-3)
- [x] Initialize repository, set up CI/CD
- [x] Apply Apache 2.0 licensing
- [x] Implement basic UI shell
- [x] Implement STL import/export
- [x] Develop basic 3-axis slicer for standard thermoplastics (baseline)
- [x] Spike: integrate `opencascade-rs` minimally to de-risk the CAD kernel dependency (`geometry/src/brep.rs`)

## Phase 2: The Fluid Solver (Months 4-7)
- [x] Implement Carreau-Yasuda rheological model
- [x] Implement Herschel-Bulkley rheological model
- [x] Implement Bingham plastic model (chocolate)
- [x] Build the 1D extrusion pressure/flow solver
- [x] Integrate material database
- [x] Build UI for rheology parameter input
- [x] Validation: unit tests against closed-form analytical solutions (Poiseuille, Herschel-Bulkley plug flow)
- [ ] Validation: regression-fit models against published literature rheometer curves (alginate, GelMA, chocolate, dough) with R² acceptance thresholds

## Phase 3: Bioprinting Specifics (Months 8-10)
- [x] Implement parametric lattice/scaffold generator (gyroid, honeycomb, Voronoi)
- [x] Add UV curing toolpath support
- [x] Add coaxial needle toolpath support
- [x] Implement safety interlocks (shear stress / thermal exposure thresholds, abort/flag logic)
- [x] Develop "slumping" visualization overlay in the UI

## Phase 4: Food Printing & Hardware (Months 11-12)
- [x] Implement thermal profiling algorithms (chocolate tempering curves)
- [x] Finalize HAL for pneumatic extruders
- [x] Finalize HAL for screw-based extruders
- [x] Build Klipper plugin implementing M300 (pneumatic pressure)
- [x] Build Klipper plugin implementing M301 (thermal profiling)
- [x] Build Klipper plugin implementing M302 (UV curing)
- [x] Build Klipper plugin implementing M303 (coaxial cross-linker)
- [ ] Beta release
- [ ] Community testing

## Ongoing / Cross-Cutting
- [ ] Confirm OpenCASCADE (LGPL-2.1 + exception) licensing compatibility before binary distribution
- [ ] Track opencascade-rs maturity/issues as integration deepens
- [ ] Acquire physical hardware (printer + rheometer) to move validation from literature-based to empirical
- [x] Marlin/RepRapFirmware support for M300-M303 (post-Klipper, community-contributed) — `marlin/`, `reprap/`, `hal/src/firmware.rs`

## Phase 5: Polish & Quality (Months 13-14)
- [x] Replace triangle-plane intersection with Sutherland-Hodgman polygon clipping for correct layer outlines
- [x] Implement geometry.json read/write in TptProject (currently dead code)
- [x] Serialize ThermalProfile for .tpt persistence
- [x] Multi-material slicer support (different materials per region)
- [x] Print time estimation from toolpath commands
- [x] Material profile CSV import for custom rheometer data
- [x] Input validation on all physical parameters (solver, UI, HAL)
- [x] Error propagation: solver errors surface as user-visible warnings, not silent skips

## Phase 6: UI & Visualization (Months 15-16)
- [x] WebGL/Three.js 3D viewport replacing flat HTML (`ui/src/viewport/index.ts`)
- [x] Real-time toolpath preview with layer-by-layer animation (`ui/src/toolpath/index.ts`)
- [x] Slumping overlay in 3D (color map per bead) (`ui/src/viewport/index.ts`)
- [x] Undo/redo for all operations (command pattern) (`ui/src/undo/index.ts`)
- [x] Material property editor with live viscosity curve plot (`ui/src/material/index.ts`)
- [x] G-code preview with syntax highlighting and line-by-line stepping (`ui/src/gcode/index.ts`)

## Phase 7: Intelligence & Automation (Months 17-18)
- [x] AI-assisted print parameter selection from geometry + material properties
  - `fluid/src/optimizer.rs` — heuristic parameter recommender (nozzle Ø, layer height, speed, flow, pressure, temperature)
  - Scoring system with confidence/reasoning per recommendation
  - Fully tested (6 unit tests)
- [x] Simulation mode: run slumping model across all layers to predict final part geometry
  - `fluid/src/simulation.rs` — layer-by-layer slumping with cumulative weight burden
  - Deformed mesh generation from simulation results
  - Fully tested (7 unit tests)
- [x] Real-time pressure/temperature feedback loop from printer to adjust parameters mid-print
  - `hal/src/feedback.rs` — PID controller with configurable gains, integral anti-windup, deadband
  - Sensor anomaly detection (spike, drift, oscillation)
  - Fully tested (8 unit tests)
- [x] Print queue management with priority scheduling
  - `core/src/queue.rs` — priority-sorted queue (4 levels), FIFO within priority
  - enqueue/start/pause/cancel/complete/priority-change/reorder operations
  - History trimming, status tracking
  - Fully tested (15 unit tests)
- [x] Quality monitoring: detect under-extrusion, stringing, layer shifts from sensor data
  - `hal/src/monitoring.rs` — real-time defect detection (extrusion ratio, flow drops, layer shifts, pressure spikes, temperature drift)
  - Quality score, alert rate, per-alert severity
  - Fully tested (10 unit tests)
- [x] Tauri backend: 20 new commands wired into `ui/src/main.rs`
- [x] Frontend: 4 new tabs (AI Optimizer, Simulation, Print Queue, Quality) in `ui/index.html`
- [x] TypeScript: wrappers in `ui/src/tauri-api.ts`, types in `ui/src/types.ts`, handlers in `ui/src/main.ts`

## Future / Community
- [x] 5-axis toolpath generation for curved mandrels (vascular bioprinting) — `slice/src/five_axis.rs`
- [x] B-Rep CAD kernel integration via opencascade-rs — `geometry/src/brep.rs`
- [x] Marlin/RepRapFirmware M300-M303 support — `marlin/`, `reprap/`, `hal/src/firmware.rs`
- [x] Plugin system for custom rheology models — `fluid/src/plugin.rs`
- [x] Export to 3MF format with material metadata — `geometry/src/three_mf.rs`