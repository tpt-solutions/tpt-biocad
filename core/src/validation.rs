// Input validation for all physical parameters
// Licensed under Apache 2.0

use crate::{Kinematics, Machine, Material, Profile, RheologyModel};

/// Validate nozzle geometry parameters.
/// Returns Ok(()) if all parameters are within acceptable ranges.
pub fn validate_nozzle_geometry(
    inlet_diameter: f64,
    outlet_diameter: f64,
    length: f64,
    taper_angle: f64,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if inlet_diameter <= 0.0 || !inlet_diameter.is_finite() {
        errors.push(format!(
            "inlet diameter must be positive, got {}",
            inlet_diameter
        ));
    }
    if outlet_diameter <= 0.0 || !outlet_diameter.is_finite() {
        errors.push(format!(
            "outlet diameter must be positive, got {}",
            outlet_diameter
        ));
    }
    if outlet_diameter > inlet_diameter {
        errors.push(format!(
            "outlet diameter ({}) must not exceed inlet diameter ({})",
            outlet_diameter, inlet_diameter
        ));
    }
    if length <= 0.0 || !length.is_finite() {
        errors.push(format!("nozzle length must be positive, got {}", length));
    }
    if !(0.0..=std::f64::consts::FRAC_PI_2).contains(&taper_angle) {
        errors.push(format!(
            "taper angle must be in [0, π/2] radians, got {}",
            taper_angle
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate flow rate parameter.
pub fn validate_flow_rate(flow_rate: f64) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();
    if flow_rate < 0.0 || !flow_rate.is_finite() {
        errors.push(format!("flow rate must be non-negative, got {}", flow_rate));
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate material parameters.
pub fn validate_material(material: &Material) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if material.name.is_empty() {
        errors.push("material name must not be empty".to_string());
    }
    if material.density <= 0.0 || !material.density.is_finite() {
        errors.push(format!(
            "density must be positive, got {}",
            material.density
        ));
    }
    if material.density > 50000.0 {
        errors.push(format!(
            "density ({}) seems unreasonably high (> 50000 kg/m³)",
            material.density
        ));
    }

    validate_rheology(&material.rheology, &mut errors);

    if let Some(ref curing) = material.curing {
        if curing.uv_intensity < 0.0 || !curing.uv_intensity.is_finite() {
            errors.push(format!(
                "UV intensity must be non-negative, got {}",
                curing.uv_intensity
            ));
        }
        if curing.exposure_time < 0.0 || !curing.exposure_time.is_finite() {
            errors.push(format!(
                "exposure time must be non-negative, got {}",
                curing.exposure_time
            ));
        }
        if curing.wavelength <= 0.0 || !curing.wavelength.is_finite() {
            errors.push(format!(
                "wavelength must be positive, got {}",
                curing.wavelength
            ));
        }
    }

    if let Some(ref coaxial) = material.coaxial {
        if coaxial.flow_ratio < 0.0 || !coaxial.flow_ratio.is_finite() {
            errors.push(format!(
                "flow ratio must be non-negative, got {}",
                coaxial.flow_ratio
            ));
        }
        if coaxial.concentration < 0.0 || !coaxial.concentration.is_finite() {
            errors.push(format!(
                "concentration must be non-negative, got {}",
                coaxial.concentration
            ));
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_rheology(model: &RheologyModel, errors: &mut Vec<String>) {
    match model {
        RheologyModel::Newtonian { viscosity } => {
            if *viscosity <= 0.0 || !viscosity.is_finite() {
                errors.push(format!(
                    "Newtonian viscosity must be positive, got {}",
                    viscosity
                ));
            }
        }
        RheologyModel::CarreauYasuda {
            eta_zero,
            eta_inf,
            lambda,
            a,
            n,
        } => {
            if *eta_zero <= 0.0 || !eta_zero.is_finite() {
                errors.push(format!("eta_zero must be positive, got {}", eta_zero));
            }
            if *eta_inf < 0.0 || !eta_inf.is_finite() {
                errors.push(format!("eta_inf must be non-negative, got {}", eta_inf));
            }
            if *eta_inf > *eta_zero {
                errors.push(format!(
                    "eta_inf ({}) must not exceed eta_zero ({})",
                    eta_inf, eta_zero
                ));
            }
            if *lambda <= 0.0 || !lambda.is_finite() {
                errors.push(format!("lambda must be positive, got {}", lambda));
            }
            if !(0.0..=1.0).contains(a) {
                errors.push(format!("Yasuda parameter a must be in (0, 1], got {}", a));
            }
            if !(0.0..=1.0).contains(n) {
                errors.push(format!("power-law index n must be in (0, 1], got {}", n));
            }
        }
        RheologyModel::HerschelBulkley { tau_yield, k, n } => {
            if *tau_yield < 0.0 || !tau_yield.is_finite() {
                errors.push(format!(
                    "yield stress must be non-negative, got {}",
                    tau_yield
                ));
            }
            if *k <= 0.0 || !k.is_finite() {
                errors.push(format!("consistency k must be positive, got {}", k));
            }
            if !(0.0..=1.0).contains(n) {
                errors.push(format!("power-law index n must be in (0, 1], got {}", n));
            }
        }
        RheologyModel::Bingham { tau_yield, mu_p } => {
            if *tau_yield < 0.0 || !tau_yield.is_finite() {
                errors.push(format!(
                    "yield stress must be non-negative, got {}",
                    tau_yield
                ));
            }
            if *mu_p <= 0.0 || !mu_p.is_finite() {
                errors.push(format!("plastic viscosity must be positive, got {}", mu_p));
            }
        }
        RheologyModel::Custom { plugin_key } => {
            if plugin_key.is_empty() {
                errors.push("custom plugin key must not be empty".to_string());
            }
        }
    }
}

/// Validate machine configuration.
pub fn validate_machine(machine: &Machine) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if machine.name.is_empty() {
        errors.push("machine name must not be empty".to_string());
    }
    if machine.nozzle_diameter <= 0.0 || !machine.nozzle_diameter.is_finite() {
        errors.push(format!(
            "nozzle diameter must be positive, got {}",
            machine.nozzle_diameter
        ));
    }

    match &machine.kinematics {
        Kinematics::Cartesian { build_volume }
        | Kinematics::CoreXY { build_volume }
        | Kinematics::Delta { build_volume } => {
            let (x, y, z) = build_volume;
            if *x <= 0.0 || !x.is_finite() {
                errors.push(format!("build volume X must be positive, got {}", x));
            }
            if *y <= 0.0 || !y.is_finite() {
                errors.push(format!("build volume Y must be positive, got {}", y));
            }
            if *z <= 0.0 || !z.is_finite() {
                errors.push(format!("build volume Z must be positive, got {}", z));
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Validate profile parameters.
pub fn validate_profile(profile: &Profile) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    if profile.layer_height <= 0.0 || !profile.layer_height.is_finite() {
        errors.push(format!(
            "layer height must be positive, got {}",
            profile.layer_height
        ));
    }
    if profile.print_speed <= 0.0 || !profile.print_speed.is_finite() {
        errors.push(format!(
            "print speed must be positive, got {}",
            profile.print_speed
        ));
    }
    if !(0.0..=1.0).contains(&profile.infill_density) || !profile.infill_density.is_finite() {
        errors.push(format!(
            "infill density must be in [0, 1], got {}",
            profile.infill_density
        ));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_nozzle_ok() {
        assert!(validate_nozzle_geometry(1.0, 0.4, 10.0, 0.0).is_ok());
    }

    #[test]
    fn test_validate_nozzle_bad_outlet() {
        let result = validate_nozzle_geometry(1.0, -0.4, 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_nozzle_outlet_too_big() {
        let result = validate_nozzle_geometry(1.0, 2.0, 10.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_flow_rate_ok() {
        assert!(validate_flow_rate(100.0).is_ok());
    }

    #[test]
    fn test_validate_flow_rate_negative() {
        assert!(validate_flow_rate(-1.0).is_err());
    }

    #[test]
    fn test_validate_material_ok() {
        let mat = Material {
            name: "Test".to_string(),
            density: 1000.0,
            rheology: RheologyModel::Newtonian { viscosity: 1.0 },
            curing: None,
            coaxial: None,
        };
        assert!(validate_material(&mat).is_ok());
    }

    #[test]
    fn test_validate_material_bad_density() {
        let mat = Material {
            name: "Test".to_string(),
            density: -1.0,
            rheology: RheologyModel::Newtonian { viscosity: 1.0 },
            curing: None,
            coaxial: None,
        };
        assert!(validate_material(&mat).is_err());
    }
}
