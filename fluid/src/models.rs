// Rheological models
// Licensed under Apache 2.0

use tpt_core::RheologyModel;

/// Calculate apparent viscosity for a given shear rate
pub fn viscosity(model: &RheologyModel, shear_rate: f64) -> f64 {
    match model {
        RheologyModel::Newtonian { viscosity } => *viscosity,
        
        RheologyModel::CarreauYasuda {
            eta_zero,
            eta_inf,
            lambda,
            a,
            n,
        } => {
            // Carreau-Yasuda model: η(γ̇) = η∞ + (η₀ - η∞) * (1 + (λγ̇)^a)^((n-1)/a)
            let lambda_gamma = lambda * shear_rate;
            let term = (1.0 + lambda_gamma.powf(*a)).powf((*n - 1.0) / a);
            eta_inf + (eta_zero - eta_inf) * term
        }
        
        RheologyModel::HerschelBulkley { tau_yield, k, n } => {
            // Herschel-Bulkley: τ = τy + kγ̇^n, so η = τ/γ̇ = τy/γ̇ + kγ̇^(n-1)
            if shear_rate == 0.0 {
                f64::INFINITY
            } else if shear_rate * tau_yield / (shear_rate * k * shear_rate.powf(*n - 1.0) + tau_yield) > 1.0 {
                // Below yield stress - no flow
                f64::INFINITY
            } else {
                tau_yield / shear_rate + k * shear_rate.powf(*n - 1.0)
            }
        }
        
        RheologyModel::Bingham { tau_yield, mu_p } => {
            // Bingham plastic: τ = τy + μpγ̇, so η = τ/γ̇ = τy/γ̇ + μp
            if shear_rate == 0.0 {
                f64::INFINITY
            } else {
                tau_yield / shear_rate + mu_p
            }
        }
    }
}

/// Calculate shear stress for a given shear rate
pub fn shear_stress(model: &RheologyModel, shear_rate: f64) -> f64 {
    match model {
        RheologyModel::Newtonian { viscosity } => viscosity * shear_rate,
        
        RheologyModel::CarreauYasuda {
            eta_zero,
            eta_inf,
            lambda,
            a,
            n,
        } => {
            // τ = η(γ̇) * γ̇
            viscosity(model, shear_rate) * shear_rate
        }
        
        RheologyModel::HerschelBulkley { tau_yield, k, n } => {
            // τ = τy + kγ̇^n
            if shear_rate == 0.0 {
                0.0
            } else {
                tau_yield + k * shear_rate.powf(*n)
            }
        }
        
        RheologyModel::Bingham { tau_yield, mu_p } => {
            // τ = τy + μpγ̇
            tau_yield + mu_p * shear_rate
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_newtonian_viscosity() {
        let model = RheologyModel::Newtonian { viscosity: 1.0 };
        assert_relative_eq!(viscosity(&model, 100.0), 1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_bingham_viscosity() {
        let model = RheologyModel::Bingham { tau_yield: 10.0, mu_p: 5.0 };
        // At high shear rate, should approach mu_p
        let eta = viscosity(&model, 1000.0);
        assert!(eta > 5.0 && eta < 6.0);
    }

    #[test]
    fn test_bingham_shear_stress() {
        let model = RheologyModel::Bingham { tau_yield: 10.0, mu_p: 5.0 };
        // τ = τy + μpγ̇
        assert_relative_eq!(shear_stress(&model, 100.0), 10.0 + 5.0 * 100.0, epsilon = 1e-10);
    }
}