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
}
