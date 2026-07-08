// Material database
// Licensed under Apache 2.0

use tpt_core::{CoaxialParams, CuringParams, Material, RheologyModel};

/// Pre-characterized materials for bioprinting and food printing
pub fn get_material(name: &str) -> Option<Material> {
    match name.to_lowercase().as_str() {
        // Bio-inks
        "alginate-4" | "alginate 4%" => Some(Material {
            name: "Alginate 4%".to_string(),
            density: 1010.0,
            rheology: RheologyModel::CarreauYasuda {
                eta_zero: 100.0,
                eta_inf: 0.1,
                lambda: 0.5,
                a: 0.5,
                n: 0.35,
            },
            curing: None,
            coaxial: Some(CoaxialParams {
                crosslinker_name: "CaCl₂".to_string(),
                flow_ratio: 0.1,
                concentration: 0.1,
            }),
        }),

        "gelma-10" | "gelma 10%" => Some(Material {
            name: "GelMA 10%".to_string(),
            density: 1020.0,
            rheology: RheologyModel::CarreauYasuda {
                eta_zero: 50.0,
                eta_inf: 0.05,
                lambda: 0.3,
                a: 0.4,
                n: 0.4,
            },
            curing: Some(CuringParams {
                uv_intensity: 10.0,
                exposure_time: 30.0,
                wavelength: 365.0,
            }),
            coaxial: None,
        }),

        "pluronic-f127" => Some(Material {
            name: "Pluronic F-127".to_string(),
            density: 1000.0,
            rheology: RheologyModel::CarreauYasuda {
                eta_zero: 200.0,
                eta_inf: 0.01,
                lambda: 1.0,
                a: 0.6,
                n: 0.25,
            },
            curing: None,
            coaxial: None,
        }),

        "fibrin" => Some(Material {
            name: "Fibrin".to_string(),
            density: 1015.0,
            rheology: RheologyModel::HerschelBulkley {
                tau_yield: 50.0,
                k: 10.0,
                n: 0.5,
            },
            curing: None,
            coaxial: None,
        }),

        // Food materials
        "dark-chocolate" | "chocolate-dark" => Some(Material {
            name: "Dark Chocolate".to_string(),
            density: 1200.0,
            rheology: RheologyModel::Bingham {
                tau_yield: 50.0,
                mu_p: 5.0,
            },
            curing: None,
            coaxial: None,
        }),

        "milk-chocolate" | "chocolate-milk" => Some(Material {
            name: "Milk Chocolate".to_string(),
            density: 1150.0,
            rheology: RheologyModel::Bingham {
                tau_yield: 30.0,
                mu_p: 3.0,
            },
            curing: None,
            coaxial: None,
        }),

        "white-chocolate" | "chocolate-white" => Some(Material {
            name: "White Chocolate".to_string(),
            density: 1100.0,
            rheology: RheologyModel::Bingham {
                tau_yield: 25.0,
                mu_p: 2.5,
            },
            curing: None,
            coaxial: None,
        }),

        "shortbread-dough" | "dough-shortbread" => Some(Material {
            name: "Shortbread Dough".to_string(),
            density: 1300.0,
            rheology: RheologyModel::HerschelBulkley {
                tau_yield: 100.0,
                k: 5.0,
                n: 0.5,
            },
            curing: None,
            coaxial: None,
        }),

        "tomato-puree" => Some(Material {
            name: "Tomato Puree".to_string(),
            density: 1030.0,
            rheology: RheologyModel::HerschelBulkley {
                tau_yield: 10.0,
                k: 1.0,
                n: 0.6,
            },
            curing: None,
            coaxial: None,
        }),

        "mashed-potato" => Some(Material {
            name: "Mashed Potato".to_string(),
            density: 1050.0,
            rheology: RheologyModel::HerschelBulkley {
                tau_yield: 15.0,
                k: 2.0,
                n: 0.45,
            },
            curing: None,
            coaxial: None,
        }),

        _ => None,
    }
}

/// List all available materials
pub fn list_materials() -> Vec<&'static str> {
    vec![
        "Alginate 4%",
        "GelMA 10%",
        "Pluronic F-127",
        "Fibrin",
        "Dark Chocolate",
        "Milk Chocolate",
        "White Chocolate",
        "Shortbread Dough",
        "Tomato Puree",
        "Mashed Potato",
    ]
}

/// Import a custom material from CSV rheometer data.
///
/// The CSV should have two columns: `shear_rate,viscosity` (no header required,
/// but a header row starting with "shear" is skipped).
///
/// The function fits a Carreau-Yasuda model to the data using a simple
/// least-squares approach and returns the resulting Material.
///
/// # Arguments
/// * `name` - Name for the custom material.
/// * `density` - Material density in kg/m³.
/// * `csv_data` - Raw CSV content as a string.
pub fn import_material_from_csv(
    name: &str,
    density: f64,
    csv_data: &str,
) -> Result<Material, String> {
    let mut data_points = Vec::new();

    for line in csv_data.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Skip header rows
        if line.to_lowercase().starts_with("shear") || line.to_lowercase().starts_with("gamma") {
            continue;
        }

        let parts: Vec<&str> = line.split([',', '\t', ';']).collect();
        if parts.len() < 2 {
            continue;
        }

        let shear_rate: f64 = parts[0]
            .trim()
            .parse()
            .map_err(|e| format!("invalid shear rate '{}': {}", parts[0].trim(), e))?;
        let visc: f64 = parts[1]
            .trim()
            .parse()
            .map_err(|e| format!("invalid viscosity '{}': {}", parts[1].trim(), e))?;

        if shear_rate > 0.0 && visc > 0.0 {
            data_points.push((shear_rate, visc));
        }
    }

    if data_points.is_empty() {
        return Err("no valid data points found in CSV".to_string());
    }

    // Fit a Carreau-Yasuda model via simple parameter search
    let model = fit_carreau_yasuda(&data_points);

    Ok(Material {
        name: name.to_string(),
        density,
        rheology: model,
        curing: None,
        coaxial: None,
    })
}

/// Simple Carreau-Yasuda fit using grid search over key parameters.
fn fit_carreau_yasuda(data: &[(f64, f64)]) -> RheologyModel {
    // Estimate eta_zero from lowest shear rate, eta_inf from highest
    let sorted = {
        let mut v = data.to_vec();
        v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        v
    };

    let eta_zero_est = sorted.first().map(|d| d.1).unwrap_or(1.0);
    let eta_inf_est = sorted.last().map(|d| d.1).unwrap_or(0.01);

    // Grid search for best lambda and n
    let mut best_err = f64::INFINITY;
    let mut best_lambda = 0.5;
    let mut best_n = 0.5;
    let best_a = 0.5; // fix a at 0.5 for simplicity

    for lambda_exp in -2..=2 {
        for n_pct in 10..=90 {
            let lambda = 10f64.powi(lambda_exp);
            let n = n_pct as f64 / 100.0;

            let err: f64 = data
                .iter()
                .map(|&(sr, obs_visc)| {
                    let lg = lambda * sr;
                    let term = (1.0 + lg.powf(best_a)).powf((n - 1.0) / best_a);
                    let pred = eta_inf_est + (eta_zero_est - eta_inf_est) * term;
                    ((pred - obs_visc) / obs_visc).powi(2)
                })
                .sum();

            if err < best_err {
                best_err = err;
                best_lambda = lambda;
                best_n = n;
            }
        }
    }

    RheologyModel::CarreauYasuda {
        eta_zero: eta_zero_est,
        eta_inf: eta_inf_est,
        lambda: best_lambda,
        a: best_a,
        n: best_n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_alginate() {
        let material = get_material("alginate-4").unwrap();
        assert_eq!(material.name, "Alginate 4%");
        assert_eq!(material.density, 1010.0);
    }

    #[test]
    fn test_get_chocolate() {
        let material = get_material("dark-chocolate").unwrap();
        assert_eq!(material.name, "Dark Chocolate");

        match &material.rheology {
            RheologyModel::Bingham { tau_yield, mu_p } => {
                assert_eq!(*tau_yield, 50.0);
                assert_eq!(*mu_p, 5.0);
            }
            _ => panic!("Expected Bingham model for chocolate"),
        }
    }

    #[test]
    fn test_list_materials() {
        let materials = list_materials();
        assert!(!materials.is_empty());
        assert!(materials.contains(&"Alginate 4%"));
    }

    #[test]
    fn test_csv_import() {
        let csv = "shear_rate,viscosity\n0.1,95.0\n1.0,60.0\n10.0,20.0\n100.0,5.0\n1000.0,1.0\n";
        let material = import_material_from_csv("Custom Bio-ink", 1015.0, csv).unwrap();
        assert_eq!(material.name, "Custom Bio-ink");
        assert_eq!(material.density, 1015.0);
        match &material.rheology {
            RheologyModel::CarreauYasuda {
                eta_zero, eta_inf, ..
            } => {
                assert!(*eta_zero > 50.0, "eta_zero should be ~95, got {}", eta_zero);
                assert!(*eta_inf < 5.0, "eta_inf should be ~1, got {}", eta_inf);
            }
            _ => panic!("Expected Carreau-Yasuda model"),
        }
    }

    #[test]
    fn test_csv_import_empty() {
        let result = import_material_from_csv("Empty", 1000.0, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_csv_import_tab_delimited() {
        let csv = "0.1\t95.0\n1.0\t60.0\n10.0\t20.0\n";
        let material = import_material_from_csv("Tab Data", 1000.0, csv).unwrap();
        assert_eq!(material.name, "Tab Data");
    }
}
