// 1D extrusion pressure/flow solver
// Licensed under Apache 2.0

use crate::viscosity;
use serde::{Deserialize, Serialize};
use tpt_core::RheologyModel;

/// Nozzle geometry parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NozzleGeometry {
    pub inlet_diameter: f64,  // mm
    pub outlet_diameter: f64, // mm
    pub length: f64,          // mm
    pub taper_angle: f64,     // radians (0 for cylindrical)
}

impl Default for NozzleGeometry {
    fn default() -> Self {
        Self {
            inlet_diameter: 1.0,
            outlet_diameter: 0.4,
            length: 10.0,
            taper_angle: 0.0,
        }
    }
}

/// Result of pressure/flow calculation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowResult {
    pub pressure_drop: f64,      // Pa
    pub flow_rate: f64,          // mm³/s
    pub wall_shear_stress: f64,  // Pa
    pub average_shear_rate: f64, // 1/s
}

/// Solve for pressure drop given target flow rate.
///
/// For cylindrical nozzles this uses the exact Hagen-Poiseuille relation for
/// Newtonian fluids and a robust bisection on the wall shear stress combined
/// with the Mooney-Rabinowitsch integral for generalized non-Newtonian fluids.
/// For tapered nozzles it falls back to a segment-wise numerical integration.
pub fn solve_pressure(
    model: &RheologyModel,
    geometry: &NozzleGeometry,
    target_flow_rate: f64,  // mm³/s
    _ambient_pressure: f64, // Pa (reserved for absolute-pressure reporting)
) -> Result<FlowResult, &'static str> {
    // Validate geometry
    if geometry.outlet_diameter <= 0.0 {
        return Err("outlet diameter must be positive");
    }
    if geometry.length <= 0.0 {
        return Err("nozzle length must be positive");
    }
    if geometry.inlet_diameter <= 0.0 {
        return Err("inlet diameter must be positive");
    }
    if target_flow_rate < 0.0 {
        return Err("flow rate must be non-negative");
    }

    // Convert flow rate to m³/s
    let q = target_flow_rate * 1e-9;

    if geometry.taper_angle == 0.0 {
        solve_cylindrical(model, geometry, q)
    } else {
        solve_tapered(model, geometry, q)
    }
}

/// Invert the constitutive relation: given a shear stress `tau` (Pa), return
/// the corresponding shear rate (1/s).
fn shear_rate_from_stress(model: &RheologyModel, tau: f64) -> f64 {
    if tau <= 0.0 {
        return 0.0;
    }
    match model {
        RheologyModel::Newtonian { viscosity } => tau / viscosity,
        RheologyModel::Bingham { tau_yield, mu_p } => {
            if tau <= *tau_yield {
                0.0
            } else {
                (tau - tau_yield) / mu_p
            }
        }
        RheologyModel::HerschelBulkley { tau_yield, k, n } => {
            if tau <= *tau_yield {
                0.0
            } else {
                ((tau - tau_yield) / k).powf(1.0 / n)
            }
        }
        RheologyModel::CarreauYasuda { .. } => {
            // Numerically invert τ = η(γ̇)·γ̇ via bisection on γ̇.
            let mut lo = 0.0;
            let mut hi = 1e8;
            for _ in 0..100 {
                let mid = 0.5 * (lo + hi);
                let t = viscosity(model, mid) * mid;
                if t < tau {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            0.5 * (lo + hi)
        }
    }
}

/// Volumetric flow rate (m³/s) through a cylindrical pipe of the given radius
/// for a specified wall shear stress, using the Mooney-Rabinowitsch integral:
///   Q = (π R³ / τw³) ∫₀^τw τ² γ̇(τ) dτ
fn flow_rate_from_wall_stress(model: &RheologyModel, tau_w: f64, radius: f64) -> f64 {
    if tau_w <= 0.0 {
        return 0.0;
    }
    let n = 400usize;
    let dtau = tau_w / n as f64;
    let mut integral = 0.0;
    for i in 1..=n {
        let tau = tau_w * (i as f64) / (n as f64);
        let gdot = shear_rate_from_stress(model, tau);
        integral += tau * tau * gdot;
    }
    integral *= dtau;
    std::f64::consts::PI * radius.powi(3) * integral / tau_w.powi(3)
}

/// Cylindrical nozzle solver.
fn solve_cylindrical(
    model: &RheologyModel,
    geometry: &NozzleGeometry,
    q: f64,
) -> Result<FlowResult, &'static str> {
    let radius = geometry.outlet_diameter / 2.0 * 1e-3; // m
    let length = geometry.length * 1e-3; // m

    match model {
        RheologyModel::Newtonian { viscosity } => {
            // ΔP = 8ηQL/(πR⁴)
            let pressure_drop =
                8.0 * viscosity * q * length / (std::f64::consts::PI * radius.powi(4));
            let wall_shear_stress = 4.0 * viscosity * q / (std::f64::consts::PI * radius.powi(3));
            let avg_shear_rate = 4.0 * q / (std::f64::consts::PI * radius.powi(3));
            Ok(FlowResult {
                pressure_drop,
                flow_rate: q * 1e9,
                wall_shear_stress,
                average_shear_rate: avg_shear_rate,
            })
        }
        _ => {
            // Bisection on wall shear stress to match the target flow rate.
            // τw relates to pressure drop by ΔP = 2 τw L / R.
            let mut lo = 0.0;
            let mut hi = 1e9; // generous upper bound (Pa)
            for _ in 0..200 {
                let mid = 0.5 * (lo + hi);
                let q_mid = flow_rate_from_wall_stress(model, mid, radius);
                if q_mid < q {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            let tau_w = 0.5 * (lo + hi);
            let pressure_drop = 2.0 * tau_w * length / radius;
            let avg_shear_rate = 4.0 * q / (std::f64::consts::PI * radius.powi(3));
            Ok(FlowResult {
                pressure_drop,
                flow_rate: q * 1e9,
                wall_shear_stress: tau_w,
                average_shear_rate: avg_shear_rate,
            })
        }
    }
}

/// Tapered nozzle solver (numerical integration).
fn solve_tapered(
    model: &RheologyModel,
    geometry: &NozzleGeometry,
    q: f64,
) -> Result<FlowResult, &'static str> {
    // Numerical integration along nozzle length.
    let n_segments = 100;
    let mut total_pressure: f64 = 0.0;
    let mut max_shear_stress: f64 = 0.0;

    let inlet_radius = geometry.inlet_diameter / 2.0 * 1e-3;
    let outlet_radius = geometry.outlet_diameter / 2.0 * 1e-3;
    let length = geometry.length * 1e-3;

    for i in 0..n_segments {
        let t = (i as f64) / (n_segments as f64);
        let t_next = ((i + 1) as f64) / (n_segments as f64);

        // Linear taper
        let r1 = inlet_radius + t * (outlet_radius - inlet_radius);
        let r2 = inlet_radius + t_next * (outlet_radius - inlet_radius);
        let r_avg = (r1 + r2) / 2.0;
        let dl = length / n_segments as f64;

        // Estimate shear rate at this segment (Newtonian-like wall estimate)
        let shear_rate = 8.0 * q / (std::f64::consts::PI * r_avg.powi(2) * dl);
        let eta = viscosity(model, shear_rate);

        // Pressure drop for this segment
        total_pressure += 8.0 * eta * q * dl / (std::f64::consts::PI * r_avg.powi(4));

        // Wall shear stress
        let tau_wall = 4.0 * eta * q / (std::f64::consts::PI * r_avg.powi(3));
        max_shear_stress = max_shear_stress.max(tau_wall);
    }

    let avg_shear_rate = 8.0 * q / (std::f64::consts::PI * outlet_radius.powi(2) * length);

    Ok(FlowResult {
        pressure_drop: total_pressure,
        flow_rate: q * 1e9,
        wall_shear_stress: max_shear_stress,
        average_shear_rate: avg_shear_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_newtonian_pressure() {
        let model = RheologyModel::Newtonian { viscosity: 1.0 };
        let geometry = NozzleGeometry {
            inlet_diameter: 1.0,
            outlet_diameter: 0.4,
            length: 10.0,
            taper_angle: 0.0,
        };

        let result = solve_pressure(&model, &geometry, 100.0, 101325.0).unwrap();

        // For Newtonian: ΔP = 8ηQL/(πR⁴)
        // Q = 100 mm³/s = 1e-7 m³/s
        // R = 0.2 mm = 2e-4 m
        // L = 10 mm = 0.01 m
        let expected_dp = 8.0 * 1.0 * 1e-7 * 0.01 / (std::f64::consts::PI * (2e-4_f64).powi(4));
        assert_relative_eq!(result.pressure_drop, expected_dp, epsilon = 1e-6);
    }
}
