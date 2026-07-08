// Core types for TPT BioCAD
// Licensed under Apache 2.0

use serde::{Deserialize, Serialize};

/// UV curing parameters for photo-crosslinkable bio-inks (e.g., GelMA).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CuringParams {
    /// UV intensity (W/m²).
    pub uv_intensity: f64,
    /// Exposure time per layer (seconds).
    pub exposure_time: f64,
    /// Wavelength (nm), typically 365 for GelMA.
    pub wavelength: f64,
}

/// Coaxial needle parameters for dual-channel extrusion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoaxialParams {
    /// Name of the cross-linker material (e.g., "CaCl₂").
    pub crosslinker_name: String,
    /// Cross-linker-to-bioink volumetric flow ratio.
    pub flow_ratio: f64,
    /// Cross-linker concentration (mol/L).
    pub concentration: f64,
}

/// Material rheological parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    pub density: f64, // kg/m³
    pub rheology: RheologyModel,
    /// UV curing parameters, if the material requires photo-crosslinking.
    #[serde(default)]
    pub curing: Option<CuringParams>,
    /// Coaxial needle parameters, if the material requires dual-channel extrusion.
    #[serde(default)]
    pub coaxial: Option<CoaxialParams>,
}

/// Rheological model types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RheologyModel {
    /// Newtonian fluid: viscosity is constant
    Newtonian { viscosity: f64 }, // Pa·s

    /// Carreau-Yasuda model for shear-thinning bio-inks
    CarreauYasuda {
        eta_zero: f64, // Zero-shear viscosity (Pa·s)
        eta_inf: f64,  // Infinite-shear viscosity (Pa·s)
        lambda: f64,   // Time constant (s)
        a: f64,        // Yasuda parameter (dimensionless)
        n: f64,        // Power-law index (dimensionless)
    },

    /// Herschel-Bulkley model for yield-stress food pastes
    HerschelBulkley {
        tau_yield: f64, // Yield stress (Pa)
        k: f64,         // Consistency (Pa·s^n)
        n: f64,         // Power-law index (dimensionless)
    },

    /// Bingham plastic model for chocolate
    Bingham {
        tau_yield: f64, // Yield stress (Pa)
        mu_p: f64,      // Plastic viscosity (Pa·s)
    },
}

/// Printer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Machine {
    pub name: String,
    pub kinematics: Kinematics,
    pub nozzle_diameter: f64, // mm
    pub extruder_type: ExtruderType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Kinematics {
    Cartesian { build_volume: (f64, f64, f64) },
    CoreXY { build_volume: (f64, f64, f64) },
    Delta { build_volume: (f64, f64, f64) },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtruderType {
    Pneumatic,
    Piston,
    Screw,
}

/// Slicing profile parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub layer_height: f64,
    pub print_speed: f64,
    pub infill_density: f64,
    pub infill_pattern: InfillPattern,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InfillPattern {
    Grid,
    Gyroid,
    Honeycomb,
    Voronoi,
}
