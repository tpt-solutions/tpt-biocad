// Slumping prediction for extruded beads
// Licensed under Apache 2.0
//
// Predicts how an extruded bead spreads or slumps on the previous layer based
// on the material's yield stress, viscosity, and print speed. This drives the
// "slumping visualization overlay" in the UI (spec Phase 3).

use tpt_core::RheologyModel;

/// Result of a slumping prediction for a single bead.
#[derive(Debug, Clone)]
pub struct SlumpResult {
    /// Bead width after settling (mm). Starts at `initial_width`.
    pub width_after: f64,
    /// Bead height after settling (mm). Starts at `initial_height`.
    pub height_after: f64,
    /// Estimated time for the bead to reach equilibrium (s).
    pub settling_time: f64,
    /// Slump factor: 0 = no deformation, 1 = full collapse to a flat film.
    pub slump_factor: f64,
}

/// Predict slumping of an extruded bead on a flat substrate.
///
/// For yield-stress fluids (Herschel-Bulkley, Bingham), gravitational stress
/// is compared against the yield stress. If ρgh < τ_yield the bead holds its
/// shape; if ρgh > τ_yield it collapses proportionally.
///
/// For shear-thinning / Newtonian fluids, a viscous relaxation model is used
/// where the slump factor depends on the ratio of gravitational driving force
/// to viscous resistance.
///
/// # Arguments
/// * `model` - Rheological model of the material.
/// * `density` - Material density (kg/m³).
/// * `initial_width` - Extruded bead width before slumping (mm).
/// * `initial_height` - Extruded bead height before slumping (mm).
/// * `print_speed` - Print speed (mm/s). Faster speeds give less time to slump
///   before the next layer is deposited.
/// * `layer_time` - Time since the previous layer was deposited (s). Longer
///   times allow more slumping.
pub fn predict_slump(
    model: &RheologyModel,
    density: f64,
    initial_width: f64,
    initial_height: f64,
    _print_speed: f64,
    layer_time: f64,
) -> SlumpResult {
    let g = 9.81; // m/s²
    let h_m = initial_height * 1e-3; // convert mm to m
    let gravitational_stress = density * g * h_m; // Pa

    let slump_factor = match model {
        RheologyModel::HerschelBulkley { tau_yield, .. }
        | RheologyModel::Bingham { tau_yield, .. } => {
            // Yield-stress model: bead holds if gravitational stress < yield stress.
            if *tau_yield <= 0.0 {
                1.0
            } else {
                (gravitational_stress / tau_yield).min(1.0)
            }
        }
        RheologyModel::Newtonian { viscosity } => {
            // Viscous relaxation: characteristic time ~ η / (ρgh).
            // Slump factor = 1 - exp(-t / τ_char), clamped to [0, 1).
            if *viscosity <= 0.0 || layer_time <= 0.0 {
                0.0
            } else {
                let tau_char = viscosity / (density * g * h_m);
                (1.0 - (-layer_time / tau_char).exp()).min(0.95)
            }
        }
        RheologyModel::CarreauYasuda { eta_zero, .. } => {
            // Use zero-shear viscosity as a conservative estimate.
            if *eta_zero <= 0.0 || layer_time <= 0.0 {
                0.0
            } else {
                let tau_char = eta_zero / (density * g * h_m);
                (1.0 - (-layer_time / tau_char).exp()).min(0.95)
            }
        }
    };

    // Conservation of area (2D cross-section): w * h = w0 * h0.
    // After slumping, the bead spreads: w = w0 * (1 + factor), h = h0 / (1 + factor).
    let spread = 1.0 + slump_factor;
    let width_after = initial_width * spread;
    let height_after = initial_height / spread;

    // Settling time estimate: for yield-stress fluids, settling is effectively
    // instantaneous once the yield stress is exceeded (塑性 flow). For viscous
    // fluids, use 5× the characteristic time (99.3% of final state).
    let settling_time = match model {
        RheologyModel::HerschelBulkley { tau_yield, .. }
        | RheologyModel::Bingham { tau_yield, .. } => {
            if gravitational_stress > *tau_yield {
                // Plastic flow: fast settling, estimate ~0.1s
                0.1
            } else {
                // Below yield: no settling
                f64::INFINITY
            }
        }
        RheologyModel::Newtonian { viscosity } => {
            if *viscosity > 0.0 {
                5.0 * viscosity / (density * g * h_m)
            } else {
                0.0
            }
        }
        RheologyModel::CarreauYasuda { eta_zero, .. } => {
            if *eta_zero > 0.0 {
                5.0 * eta_zero / (density * g * h_m)
            } else {
                0.0
            }
        }
    };

    SlumpResult {
        width_after,
        height_after,
        settling_time,
        slump_factor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_high_yield_stress_no_slump() {
        // High yield stress (dough): bead should hold its shape.
        let model = RheologyModel::HerschelBulkley {
            tau_yield: 500.0, // 500 Pa — very stiff
            k: 5.0,
            n: 0.5,
        };
        let result = predict_slump(&model, 1300.0, 0.4, 0.2, 10.0, 1.0);
        assert!(
            result.slump_factor < 0.1,
            "expected minimal slump, got {}",
            result.slump_factor
        );
        assert!(result.width_after < 0.5);
    }

    #[test]
    fn test_low_yield_stress_slumps() {
        // Low yield stress (tomato puree): bead should spread significantly.
        let model = RheologyModel::HerschelBulkley {
            tau_yield: 1.0, // 1 Pa — very weak
            k: 1.0,
            n: 0.6,
        };
        let result = predict_slump(&model, 1030.0, 0.4, 0.2, 10.0, 5.0);
        assert!(
            result.slump_factor > 0.5,
            "expected significant slump, got {}",
            result.slump_factor
        );
        assert!(result.width_after > 0.5);
    }

    #[test]
    fn test_newtonian_viscous_relaxation() {
        // Newtonian fluid: slumping depends on layer_time.
        let model = RheologyModel::Newtonian { viscosity: 10.0 };
        let short = predict_slump(&model, 1000.0, 0.4, 0.2, 10.0, 0.1);
        let long = predict_slump(&model, 1000.0, 0.4, 0.2, 10.0, 10.0);
        assert!(
            long.slump_factor > short.slump_factor,
            "longer layer time should cause more slumping"
        );
    }

    #[test]
    fn test_conservation_of_area() {
        // Cross-sectional area should be approximately conserved.
        let model = RheologyModel::Bingham {
            tau_yield: 50.0,
            mu_p: 5.0,
        };
        let result = predict_slump(&model, 1200.0, 0.4, 0.2, 10.0, 1.0);
        let initial_area = 0.4 * 0.2;
        let final_area = result.width_after * result.height_after;
        assert!(
            (initial_area - final_area).abs() < 1e-10,
            "area not conserved: {} vs {}",
            initial_area,
            final_area
        );
    }

    #[test]
    fn test_zero_yield_stress_full_slump() {
        let model = RheologyModel::Bingham {
            tau_yield: 0.0,
            mu_p: 5.0,
        };
        let result = predict_slump(&model, 1000.0, 0.4, 0.2, 10.0, 1.0);
        assert_eq!(result.slump_factor, 1.0);
    }
}
