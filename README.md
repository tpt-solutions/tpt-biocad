# TPT BioCAD

Open-source CAD and slicing software for bioprinting and food printing with integrated rheology solver.

## Overview

TPT BioCAD is a unified Computer-Aided Design (CAD) and slicing software platform specifically engineered for bioprinting and food printing. Unlike traditional slicers optimized for thermoplastic FDM/FFF printing, TPT BioCAD natively integrates a computational fluid dynamics (CFD) and rheology solver.

## Features

- **Rheological Models**: Carreau-Yasuda, Herschel-Bulkley, Bingham plastic
- **STL Import/Export**: Standard mesh format support
- **3-Axis Slicing**: Basic slicer for standard thermoplastics
- **G-code Generation**: Standard and extended M300-M303 commands
- **Material Database**: Pre-characterized bio-inks and food materials

## Project Structure

```
tpt-biocad/
├── core/       # Core types and utilities
├── geometry/   # CAD and mesh processing
├── fluid/      # Rheology and fluid dynamics solver
├── slice/      # Toolpath generation
└── ui/         # Tauri-based UI
```

## Building

```bash
# Build all crates
cargo build

# Run tests
cargo test
```

## License

Apache License 2.0