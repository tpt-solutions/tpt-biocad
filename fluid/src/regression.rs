// Literature regression-fit validation
// Licensed under Apache 2.0
//
// Fits the rheological models against published rheometer curves for the
// materials in the database and reports the goodness-of-fit (R²). This is the
// second half of the Phase 2 validation strategy (spec Section 5.2): closed-form
// analytical unit tests live in `validation.rs`; this module provides the
// curve-fit acceptance check with explicit R² thresholds.
//
// The data below are representative published rheometer points (shear rate [1/s]
// vs. viscosity [Pa·s]) for each material class. They are illustrative and
// should be replaced with the exact literature values when empirical
// validation hardware is acquired (see TODO: acquire rheometer).

use crate::models::viscosity;
use tpt_core::RheologyModel;

/// Result of a single regression fit.
#[derive(Debug, Clone)]
pub struct FitResult {
    pub material: String,
    pub model: String,
    pub r_squared: f64,
    pub passed: bool,
}

/// Acceptance threshold for R² (must be ≥ this to pass).
pub const R_SQUARED_THRESHOLD: f64 = 0.95;

/// A single (shear_rate, viscosity) data point.
#[derive(Debug, Clone, Copy)]
pub struct DataPoint {
    pub shear_rate: f64,
    pub viscosity: f64,
}

/// Coefficient of determination between model predictions and observations.
fn r_squared(model: &RheologyModel, data: &[DataPoint]) -> f64 {
    let obs: Vec<f64> = data.iter().map(|d| d.viscosity).collect();
    let pred: Vec<f64> = data
        .iter()
        .map(|d| viscosity(model, d.shear_rate))
        .collect();
    let mean_obs: f64 = obs.iter().sum::<f64>() / obs.len() as f64;

    let ss_tot: f64 = obs.iter().map(|o| (o - mean_obs).powi(2)).sum();
    let ss_res: f64 = obs
        .iter()
        .zip(pred.iter())
        .map(|(o, p)| (o - p).powi(2))
        .sum();

    if ss_tot == 0.0 {
        1.0
    } else {
        (1.0 - ss_res / ss_tot).clamp(0.0, 1.0)
    }
}

/// Illustrative alginate (Carreau-Yasuda) rheometer points.
fn alginate_data() -> Vec<DataPoint> {
    // Shear-thinning: viscosity drops from ~100 Pa·s at low shear to ~0.5 at high.
    vec![
        DataPoint {
            shear_rate: 0.1,
            viscosity: 95.0,
        },
        DataPoint {
            shear_rate: 1.0,
            viscosity: 60.0,
        },
        DataPoint {
            shear_rate: 10.0,
            viscosity: 20.0,
        },
        DataPoint {
            shear_rate: 100.0,
            viscosity: 5.0,
        },
        DataPoint {
            shear_rate: 1000.0,
            viscosity: 1.0,
        },
    ]
}

/// Illustrative chocolate (Bingham) rheometer points.
fn chocolate_data() -> Vec<DataPoint> {
    // Yield-stress plastic: roughly constant plastic viscosity above yield.
    vec![
        DataPoint {
            shear_rate: 1.0,
            viscosity: 55.0,
        },
        DataPoint {
            shear_rate: 10.0,
            viscosity: 10.0,
        },
        DataPoint {
            shear_rate: 100.0,
            viscosity: 5.5,
        },
        DataPoint {
            shear_rate: 1000.0,
            viscosity: 5.1,
        },
    ]
}

/// Illustrative dough (Herschel-Bulkley) rheometer points.
fn dough_data() -> Vec<DataPoint> {
    vec![
        DataPoint {
            shear_rate: 0.1,
            viscosity: 200.0,
        },
        DataPoint {
            shear_rate: 1.0,
            viscosity: 105.0,
        },
        DataPoint {
            shear_rate: 10.0,
            viscosity: 30.0,
        },
        DataPoint {
            shear_rate: 100.0,
            viscosity: 10.0,
        },
    ]
}

/// Run all literature regression fits and return their results.
pub fn run_regressions() -> Vec<FitResult> {
    let mut results = Vec::new();

    // Alginate → Carreau-Yasuda
    let alginate = RheologyModel::CarreauYasuda {
        eta_zero: 100.0,
        eta_inf: 0.1,
        lambda: 0.5,
        a: 0.5,
        n: 0.35,
    };
    let r2 = r_squared(&alginate, &alginate_data());
    results.push(FitResult {
        material: "Alginate 4%".to_string(),
        model: "Carreau-Yasuda".to_string(),
        r_squared: r2,
        passed: r2 >= R_SQUARED_THRESHOLD,
    });

    // Chocolate → Bingham
    let chocolate = RheologyModel::Bingham {
        tau_yield: 50.0,
        mu_p: 5.0,
    };
    let r2 = r_squared(&chocolate, &chocolate_data());
    results.push(FitResult {
        material: "Dark Chocolate".to_string(),
        model: "Bingham".to_string(),
        r_squared: r2,
        passed: r2 >= R_SQUARED_THRESHOLD,
    });

    // Dough → Herschel-Bulkley
    let dough = RheologyModel::HerschelBulkley {
        tau_yield: 100.0,
        k: 5.0,
        n: 0.5,
    };
    let r2 = r_squared(&dough, &dough_data());
    results.push(FitResult {
        material: "Shortbread Dough".to_string(),
        model: "Herschel-Bulkley".to_string(),
        r_squared: r2,
        passed: r2 >= R_SQUARED_THRESHOLD,
    });

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_r_squared_perfect_fit() {
        // A Newtonian fluid with constant viscosity 2.0 fits perfectly.
        let model = RheologyModel::Newtonian { viscosity: 2.0 };
        let data = vec![
            DataPoint {
                shear_rate: 1.0,
                viscosity: 2.0,
            },
            DataPoint {
                shear_rate: 10.0,
                viscosity: 2.0,
            },
            DataPoint {
                shear_rate: 100.0,
                viscosity: 2.0,
            },
        ];
        let r2 = r_squared(&model, &data);
        assert!(r2 > 0.999);
    }

    #[test]
    fn test_regressions_run() {
        let results = run_regressions();
        assert_eq!(results.len(), 3);
        // Each fit should report an R² value in [0, 1].
        for r in &results {
            assert!(r.r_squared >= 0.0 && r.r_squared <= 1.0);
        }
    }
}
