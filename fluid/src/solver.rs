// 1D extrusion pressure/flow solver
// Licensed under Apache 2.0

use tpt_core::RheologyModel;
use crate::viscosity;
use serde::{Deserialize, Serialize};

/// Nozzle geometry parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NozzleGeometry {
    pub inlet_diameter: f64,    // mm
    pub outlet_diameter: f64,   // mm
    pub length: f64,            // mm
    pub taper_angle: f64,       // radians (0 for cylindrical)
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
    pub pressure_drop: f64,     // Pa
    pub flow_rate: f64,         // mm³/s
    pub wall_shear_stress: f64, // Pa
    pub average_shear_rate: f64, // 1/s
}

/// Solve for pressure drop given target flow rate
/// Uses iterative approach for non-Newtonian fluids
pub fn solve_pressure(
    model: &RheologyModel,
    geometry: &NozzleGeometry,
    target_flow_rate: f64,  // mm³/s
    ambient_pressure: f64,  // Pa
) -> Result<FlowResult, &'static str> {
    // Convert flow rate to m³/s
    let q = target_flow_rate * 1e-9;
    
    // For cylindrical nozzle, use Hagen-Poiseuille based approach
    // For tapered nozzle, use numerical integration
    
    if geometry.taper_angle == 0.0 {
        // Cylindrical approximation
        solve_cylindrical(model, geometry, q)
    } else {
        // Tapered nozzle - numerical approach
        solve_tapered(model, geometry, q)
    }
}

/// Cylindrical nozzle solver
fn solve_cylindrical(
    model: &RheologyModel,
    geometry: &NozzleGeometry,
    q: f64,
) -> Result<FlowResult, &'static str> {
    let radius = geometry.outlet_diameter / 2.0 * 1e-3; // m
    let length = geometry.length * 1e-3; // m
    
    // For Newtonian: ΔP = 8ηQL/(πR⁴)
    // For non-Newtonian, we need to iterate
    
    match model {
        RheologyModel::Newtonian { viscosity } => {
            let pressure_drop = 8.0 * viscosity * q * length / (std::f64::consts::PI * radius.powi(4));
            let wall_shear_stress = 4.0 * viscosity * q / (std::f64::consts::PI * radius.powi(3));
            let avg_shear_rate = 8.0 * q / (std::f64::consts::PI * radius.powi(2) * length);
            
            Ok(FlowResult {
                pressure_drop,
                flow_rate: q * 1e9,
                wall_shear_stress,
                average_shear_rate: avg_shear_rate,
            })
        }
        
        RheologyModel::CarreauYasuda { .. } 
        | RheologyModel::HerschelBulkley { .. } 
        | RheologyModel::Bingham { .. } => {
            // Iterative solution for non-Newtonian fluids
            // Use Rabinowitsch-Mooney equation
            solve_iterative(model, radius, length, q)
        }
    }
}

/// Iterative solver for non-Newtonian fluids
fn solve_iterative(
    model: &RheologyModel,
    radius: f64,
    length: f64,
    q: f64,
) -> Result<FlowResult, &'static str> {
    // Initial guess: assume Newtonian with average viscosity
    let mut shear_rate = 8.0 * q / (std::f64::consts::PI * radius.powi(2) * length);
    
    for _ in 0..100 {
        let eta = viscosity(model, shear_rate);
        let new_shear_rate = 8.0 * q / (std::f64::consts::PI * radius.powi(2) * length)
            + 4.0 * q * eta / (std::f64::consts::PI * radius.powi(3) * shear_rate);
        
        if (new_shear_rate - shear_rate).abs() < 1e-10 {
            let pressure_drop = 8.0 * eta * q * length / (std::f64::consts::PI * radius.powi(4));
            let wall_shear_stress = 4.0 * eta * q / (std::f64::consts::PI * radius.powi(3));
            
            return Ok(FlowResult {
                pressure_drop,
                flow_rate: q * 1e9,
                wall_shear_stress,
                average_shear_rate: shear_rate,
            });
        }
        
        shear_rate = new_shear_rate;
    }
    
    Err("Failed to converge")
}

/// Tapered nozzle solver (numerical integration)
fn solve_tapered(
    model: &RheologyModel,
    geometry: &NozzleGeometry,
    q: f64,
) -> Result<FlowResult, &'static str> {
    // Numerical integration along nozzle length
    // Divide into segments and solve each segment
    let n_segments = 100;
    let mut total_pressure = 0.0;
    let mut max_shear_stress = 0.0;
    
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
        
        // Estimate shear rate at this segment
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
        // Q = 100 mm³/s = 1e-10 m³/s
        // R = 0.2 mm = 2e-4 m
        // L = 10 mm = 0.01 m
        let expected_dp = 8.0 * 1.0 * 1e-10 * 0.01 / (std::f64::consts::PI * (2e-4_f64).powi(4));
        assert_relative_eq!(result.pressure_drop, expected_dp, epsilon = 1e-10);
    }
}