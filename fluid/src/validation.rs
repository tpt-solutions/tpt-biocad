// Validation tests against analytical solutions
// Licensed under Apache 2.0

use crate::{shear_stress, solve_pressure, viscosity, NozzleGeometry};
use approx::assert_relative_eq;
use tpt_core::RheologyModel;

/// Analytical solution for Newtonian Poiseuille flow
/// ΔP = 8ηQL/(πR⁴)
fn analytical_newtonian_pressure(eta: f64, q: f64, r: f64, l: f64) -> f64 {
    8.0 * eta * q * l / (std::f64::consts::PI * r.powi(4))
}

/// Analytical solution for power-law fluid in pipe
/// For n=1 (Newtonian), this reduces to Poiseuille
/// For n≠1, uses Rabinowitsch equation
fn analytical_powerlaw_pressure(k: f64, n: f64, q: f64, r: f64, l: f64) -> f64 {
    // Wall shear rate: γ̇w = (3n+1)/(4n) * 8Q/(πR²L)
    let gamma_w = (3.0 * n + 1.0) / (4.0 * n) * 8.0 * q / (std::f64::consts::PI * r.powi(2) * l);
    // Wall shear stress: τw = k * γ̇w^n
    let tau_w = k * gamma_w.powf(n);
    // Pressure drop: ΔP = 4τwL/(3nR) * (3n+1)/(4n)
    4.0 * tau_w * l / (3.0 * n * r) * (3.0 * n + 1.0) / (4.0 * n)
}

/// Test Carreau-Yasuda model against high-shear (Newtonian) limit
#[test]
fn test_carreau_yasuda_newtonian_limit() {
    // For a shear-thinning fluid (n < 1), when λγ̇ >> 1 the model approaches η∞.
    let model = RheologyModel::CarreauYasuda {
        eta_zero: 100.0,
        eta_inf: 1.0,
        lambda: 1.0,
        a: 0.5,
        n: 0.3,
    };

    // At high shear rate (λγ̇ = 1e4 >> 1), should approach eta_inf
    let eta = viscosity(&model, 10000.0);
    assert_relative_eq!(eta, 1.0, epsilon = 0.5);
}

/// Test Carreau-Yasuda model against zero-shear limit
#[test]
fn test_carreau_yasuda_zero_shear() {
    // When λγ̇ << 1, Carreau-Yasuda should approach η₀
    let model = RheologyModel::CarreauYasuda {
        eta_zero: 100.0,
        eta_inf: 1.0,
        lambda: 100.0,
        a: 0.5,
        n: 0.35,
    };

    // At very low shear rate (λγ̇ = 1e-5 << 1), should approach eta_zero
    let eta = viscosity(&model, 1e-7);
    assert_relative_eq!(eta, 100.0, epsilon = 1.0);
}

/// Test Herschel-Bulkley model against Bingham limit
#[test]
fn test_hershel_bulkley_bingham_limit() {
    // When n=1, Herschel-Bulkley becomes Bingham
    let hb = RheologyModel::HerschelBulkley {
        tau_yield: 10.0,
        k: 5.0,
        n: 1.0,
    };

    let b = RheologyModel::Bingham {
        tau_yield: 10.0,
        mu_p: 5.0,
    };

    // At high shear rate, both should give similar results
    let eta_hb = viscosity(&hb, 1000.0);
    let eta_b = viscosity(&b, 1000.0);

    assert_relative_eq!(eta_hb, eta_b, epsilon = 0.01);
}

/// Test Newtonian pressure against analytical Poiseuille solution
#[test]
fn test_newtonian_poiseuille() {
    let model = RheologyModel::Newtonian { viscosity: 1.0 };
    let geometry = NozzleGeometry {
        inlet_diameter: 1.0,
        outlet_diameter: 0.4,
        length: 10.0,
        taper_angle: 0.0,
    };

    let result = solve_pressure(&model, &geometry, 100.0, 101325.0).unwrap();

    // Analytical solution
    let r = 0.2e-3; // 0.2 mm in meters
    let l = 10e-3; // 10 mm in meters
    let q = 100e-9; // 100 mm³/s in m³/s (matches target_flow_rate * 1e-9)

    let expected = analytical_newtonian_pressure(1.0, q, r, l);
    assert_relative_eq!(result.pressure_drop, expected, epsilon = 1e-6);
}

/// Test Carreau-Yasuda pressure calculation
#[test]
fn test_carreau_yasuda_pressure() {
    let model = RheologyModel::CarreauYasuda {
        eta_zero: 100.0,
        eta_inf: 0.1,
        lambda: 0.5,
        a: 0.5,
        n: 0.35,
    };

    let geometry = NozzleGeometry {
        inlet_diameter: 1.0,
        outlet_diameter: 0.4,
        length: 10.0,
        taper_angle: 0.0,
    };

    let result = solve_pressure(&model, &geometry, 100.0, 101325.0).unwrap();

    // Should produce a finite pressure drop
    assert!(result.pressure_drop > 0.0);
    assert!(result.pressure_drop < 1e6); // Reasonable upper bound
}

/// Test Bingham model pressure
#[test]
fn test_bingham_pressure() {
    let model = RheologyModel::Bingham {
        tau_yield: 50.0,
        mu_p: 5.0,
    };

    let geometry = NozzleGeometry {
        inlet_diameter: 1.0,
        outlet_diameter: 0.4,
        length: 10.0,
        taper_angle: 0.0,
    };

    let result = solve_pressure(&model, &geometry, 100.0, 101325.0).unwrap();

    // Bingham (μp = 5) should require higher pressure than a Newtonian fluid
    // with a much lower viscosity, because the yield stress adds resistance.
    let newtonian = RheologyModel::Newtonian { viscosity: 1.0 };
    let newtonian_result = solve_pressure(&newtonian, &geometry, 100.0, 101325.0).unwrap();

    assert!(result.pressure_drop > newtonian_result.pressure_drop);
}

/// Test Herschel-Bulkley model
#[test]
fn test_herschel_bulkley_pressure() {
    let model = RheologyModel::HerschelBulkley {
        tau_yield: 100.0,
        k: 5.0,
        n: 0.5,
    };

    let geometry = NozzleGeometry {
        inlet_diameter: 1.0,
        outlet_diameter: 0.4,
        length: 10.0,
        taper_angle: 0.0,
    };

    let result = solve_pressure(&model, &geometry, 100.0, 101325.0).unwrap();

    // Should produce a finite pressure drop
    assert!(result.pressure_drop > 0.0);
}

/// Test shear stress calculations
#[test]
fn test_shear_stress_newtonian() {
    let model = RheologyModel::Newtonian { viscosity: 1.0 };
    let tau = shear_stress(&model, 100.0);
    assert_relative_eq!(tau, 100.0, epsilon = 1e-10);
}

/// Test shear stress for Bingham model
#[test]
fn test_shear_stress_bingham() {
    let model = RheologyModel::Bingham {
        tau_yield: 10.0,
        mu_p: 5.0,
    };

    // At γ̇ = 100, τ = τy + μpγ̇ = 10 + 5*100 = 510
    let tau = shear_stress(&model, 100.0);
    assert_relative_eq!(tau, 510.0, epsilon = 1e-10);
}

/// Test shear stress for Herschel-Bulkley
#[test]
fn test_shear_stress_herschel_bulkley() {
    let model = RheologyModel::HerschelBulkley {
        tau_yield: 10.0,
        k: 5.0,
        n: 0.5,
    };

    // At γ̇ = 100, τ = τy + kγ̇^n = 10 + 5*100^0.5 = 10 + 50 = 60
    let tau = shear_stress(&model, 100.0);
    assert_relative_eq!(tau, 60.0, epsilon = 1e-10);
}
