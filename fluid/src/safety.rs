// Safety interlocks for bioprinting and food printing
// Licensed under Apache 2.0
//
// Flags or aborts a print plan when computed shear stress (nozzle wall) or
// thermal exposure exceeds a user-configurable, per-material safety threshold,
// protecting cell viability and material integrity (see spec Section 3 Goals
// and Section 5.2 Safety Interlocks).

use crate::{solve_pressure, NozzleGeometry};
use tpt_core::RheologyModel;

/// Per-material safety thresholds.
#[derive(Debug, Clone)]
pub struct SafetyLimits {
    /// Maximum allowable wall shear stress (Pa). Exceeding this risks cell
    /// damage (bio-inks) or material degradation (food pastes).
    pub max_wall_shear_stress: f64,
    /// Maximum allowable nozzle temperature (°C) for thermal exposure.
    pub max_temperature: f64,
    /// Maximum allowable cumulative thermal exposure (°C·s).
    pub max_thermal_exposure: f64,
}

impl Default for SafetyLimits {
    fn default() -> Self {
        Self {
            max_wall_shear_stress: 1000.0,
            max_temperature: 60.0,
            max_thermal_exposure: 600.0,
        }
    }
}

/// Outcome of a safety evaluation.
#[derive(Debug, Clone, PartialEq)]
pub enum SafetyStatus {
    /// All computed quantities are within limits.
    Ok,
    /// One or more quantities exceed limits; the print plan should be flagged.
    Flagged(Vec<String>),
    /// A critical limit is exceeded; the print plan should be aborted.
    Abort(Vec<String>),
}

/// Evaluate a single extrusion step against the safety limits.
///
/// `temperature` is the nozzle temperature (°C) and `dwell_time` is the time
/// (s) the material spends at that temperature for this step, used to accumulate
/// thermal exposure.
pub fn evaluate_step(
    model: &RheologyModel,
    geometry: &NozzleGeometry,
    flow_rate: f64,
    limits: &SafetyLimits,
    temperature: f64,
    dwell_time: f64,
) -> SafetyStatus {
    let mut warnings = Vec::new();

    // Wall shear stress from the 1D solver.
    match solve_pressure(model, geometry, flow_rate, 101325.0) {
        Ok(result) => {
            if result.wall_shear_stress > limits.max_wall_shear_stress {
                warnings.push(format!(
                    "wall shear stress {:.1} Pa exceeds limit {:.1} Pa",
                    result.wall_shear_stress, limits.max_wall_shear_stress
                ));
            }
        }
        Err(e) => {
            warnings.push(format!("could not compute wall shear stress: {}", e));
        }
    }

    // Temperature limit.
    if temperature > limits.max_temperature {
        warnings.push(format!(
            "temperature {:.1} °C exceeds limit {:.1} °C",
            temperature, limits.max_temperature
        ));
    }

    // Cumulative thermal exposure.
    let exposure = temperature * dwell_time;
    if exposure > limits.max_thermal_exposure {
        warnings.push(format!(
            "thermal exposure {:.1} °C·s exceeds limit {:.1} °C·s",
            exposure, limits.max_thermal_exposure
        ));
    }

    if warnings.is_empty() {
        SafetyStatus::Ok
    } else {
        // Treat temperature and thermal-exposure breaches as abort-level,
        // shear-stress breaches as flag-level (recoverable by slowing down).
        let critical =
            temperature > limits.max_temperature || exposure > limits.max_thermal_exposure;
        if critical {
            SafetyStatus::Abort(warnings)
        } else {
            SafetyStatus::Flagged(warnings)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> NozzleGeometry {
        NozzleGeometry {
            inlet_diameter: 1.0,
            outlet_diameter: 0.4,
            length: 10.0,
            taper_angle: 0.0,
        }
    }

    #[test]
    fn test_safe_step() {
        // Use a low flow rate and low viscosity so wall shear stress stays below limit.
        let model = RheologyModel::Newtonian { viscosity: 0.01 };
        let limits = SafetyLimits::default();
        let status = evaluate_step(&model, &geometry(), 1.0, &limits, 25.0, 1.0);
        assert_eq!(status, SafetyStatus::Ok);
    }

    #[test]
    fn test_high_shear_flagged() {
        let model = RheologyModel::Newtonian { viscosity: 1.0 };
        let limits = SafetyLimits {
            max_wall_shear_stress: 1.0, // very low limit
            ..Default::default()
        };
        let status = evaluate_step(&model, &geometry(), 100.0, &limits, 25.0, 1.0);
        match status {
            SafetyStatus::Flagged(w) => assert!(!w.is_empty()),
            _ => panic!("expected Flagged"),
        }
    }

    #[test]
    fn test_overheat_abort() {
        let model = RheologyModel::Newtonian { viscosity: 1.0 };
        let limits = SafetyLimits::default();
        let status = evaluate_step(
            &model,
            &geometry(),
            50.0,
            &limits,
            90.0, // above 60 °C limit
            1.0,
        );
        match status {
            SafetyStatus::Abort(w) => assert!(!w.is_empty()),
            _ => panic!("expected Abort"),
        }
    }
}
