// AI-assisted print parameter selection from geometry + material properties
// Licensed under Apache 2.0
//
// Analyzes geometry characteristics and material rheology to recommend
// optimal print parameters using heuristic scoring.

use tpt_core::{Material, RheologyModel};

/// Characteristics extracted from a 3D model for parameter selection.
#[derive(Debug, Clone)]
pub struct GeometryFeatures {
    /// Total volume of the model (mm³).
    pub volume: f64,
    /// Total surface area (mm²).
    pub surface_area: f64,
    /// Height of the model (mm).
    pub height: f64,
    /// Minimum feature size (mm), e.g., smallest wall thickness.
    pub min_feature_size: f64,
    /// Estimated maximum overhang angle (degrees from horizontal).
    pub max_overhang_angle: f64,
    /// Number of disconnected shells / parts.
    pub part_count: usize,
}

/// Recommended print parameters with confidence scores.
#[derive(Debug, Clone)]
pub struct PrintRecommendation {
    /// Recommended nozzle diameter (mm).
    pub nozzle_diameter: f64,
    /// Recommended layer height (mm).
    pub layer_height: f64,
    /// Recommended print speed (mm/s).
    pub print_speed: f64,
    /// Recommended volumetric flow rate (mm³/s).
    pub flow_rate: f64,
    /// Recommended extrusion pressure (Pa), if applicable.
    pub pressure: f64,
    /// Recommended extrusion temperature (°C), if applicable.
    pub temperature: Option<f64>,
    /// Overall confidence score (0.0 – 1.0).
    pub confidence: f64,
    /// Human-readable reasoning for each recommendation.
    pub reasoning: Vec<String>,
}

/// Scoring dimensions used in the optimizer.
#[derive(Debug, Clone)]
pub struct ParameterScores {
    pub nozzle_score: f64,
    pub layer_score: f64,
    pub speed_score: f64,
    pub flow_score: f64,
    pub overall: f64,
}

/// Analyze geometry and material to recommend print parameters.
///
/// Uses heuristic rules based on rheology class, feature size, and
/// model geometry. Returns scored recommendations with reasoning.
pub fn recommend_parameters(
    features: &GeometryFeatures,
    material: &Material,
) -> PrintRecommendation {
    let mut reasoning = Vec::new();

    let (nozzle, nozzle_reason) = recommend_nozzle(features, material);
    reasoning.push(nozzle_reason);

    let (layer_height, layer_reason) = recommend_layer_height(features, nozzle);
    reasoning.push(layer_reason);

    let (print_speed, speed_reason) = recommend_print_speed(features, material, layer_height);
    reasoning.push(speed_reason);

    let (flow_rate, flow_reason) = recommend_flow_rate(nozzle, print_speed, material);
    reasoning.push(flow_reason);

    let (pressure, pressure_reason) = estimate_pressure(material, flow_rate, nozzle);
    reasoning.push(pressure_reason);

    let temperature = estimate_temperature(material);

    let confidence = compute_confidence(
        features,
        material,
        nozzle,
        layer_height,
        print_speed,
        flow_rate,
    );

    PrintRecommendation {
        nozzle_diameter: nozzle,
        layer_height,
        print_speed,
        flow_rate,
        pressure,
        temperature,
        confidence,
        reasoning,
    }
}

fn recommend_nozzle(features: &GeometryFeatures, material: &Material) -> (f64, String) {
    let base = match &material.rheology {
        // High-viscosity / yield-stress materials need larger nozzles to avoid
        // excessive pressure drop.
        RheologyModel::HerschelBulkley { tau_yield, .. } if *tau_yield > 200.0 => {
            0.8 + 0.2 * (tau_yield / 1000.0).min(1.0)
        }
        RheologyModel::Bingham { tau_yield, .. } if *tau_yield > 100.0 => {
            0.6 + 0.3 * (tau_yield / 500.0).min(1.0)
        }
        // Shear-thinning bio-inks: standard nozzle works well.
        RheologyModel::CarreauYasuda { .. } => 0.4,
        RheologyModel::Newtonian { .. } => 0.4,
        _ => 0.6,
    };

    let nozzle = base.clamp(0.2, 1.2);
    // Constrain by minimum feature size (nozzle should not exceed feature size).
    let feature_bound = features.min_feature_size.max(0.2);
    let nozzle = nozzle.min(feature_bound);

    let reason = format!(
        "nozzle Ø={:.2}mm: {} rheology, min_feature={:.1}mm → clamped to [{:.2}, {:.2}]",
        nozzle,
        material_name_short(&material.name),
        features.min_feature_size,
        0.2,
        feature_bound,
    );
    (nozzle, reason)
}

fn recommend_layer_height(features: &GeometryFeatures, nozzle_diameter: f64) -> (f64, String) {
    // Layer height is typically 25–75% of nozzle diameter. For tall parts,
    // prefer larger layers to reduce print time. For fine detail, prefer thinner.
    let aspect = if features.height > 50.0 {
        0.6 // tall: faster
    } else if features.min_feature_size < 1.0 {
        0.3 // fine detail
    } else {
        0.5 // balance
    };
    let layer = (nozzle_diameter * aspect * 4.0).round() / 4.0;
    let layer = layer.clamp(0.05, 0.8);

    let reason = format!(
        "layer_h={:.2}mm: {:.0}% of nozzle Ø (height={:.0}mm, min_feature={:.1}mm)",
        layer,
        aspect * 100.0,
        features.height,
        features.min_feature_size,
    );
    (layer, reason)
}

fn recommend_print_speed(
    features: &GeometryFeatures,
    material: &Material,
    _layer_height: f64,
) -> (f64, String) {
    let base_speed = match &material.rheology {
        // Yield-stress fluids can be printed faster since they hold shape.
        RheologyModel::HerschelBulkley { tau_yield, .. } if *tau_yield > 200.0 => 30.0,
        RheologyModel::Bingham { tau_yield, .. } if *tau_yield > 100.0 => 25.0,
        // Shear-thinning: moderate speed.
        RheologyModel::CarreauYasuda { eta_zero, .. } => {
            if *eta_zero > 100.0 {
                15.0
            } else {
                20.0
            }
        }
        // Low-viscosity: slow down to avoid excessive slumping.
        RheologyModel::Newtonian { viscosity } => {
            if *viscosity < 1.0 {
                5.0
            } else {
                10.0
            }
        }
        _ => 15.0,
    };

    // Reduce speed for tall parts to reduce vibration risk.
    let height_factor = if features.height > 100.0 {
        0.6
    } else if features.height > 50.0 {
        0.8
    } else {
        1.0
    };

    // Reduce speed for thin features.
    let feature_factor = if features.min_feature_size < 0.5 {
        0.7
    } else {
        1.0
    };

    let speed: f64 = base_speed * height_factor * feature_factor;
    let speed = speed.clamp(1.0, 60.0);

    let reason = format!(
        "speed={:.0}mm/s: base={:.0} (from {}), height_factor={:.1}, feature_factor={:.1}",
        speed,
        base_speed,
        material_name_short(&material.name),
        height_factor,
        feature_factor,
    );
    (speed, reason)
}

fn recommend_flow_rate(
    nozzle_diameter: f64,
    print_speed: f64,
    material: &Material,
) -> (f64, String) {
    // Target cross-sectional area = nozzle_diameter * layer_height ≈ circular approximation.
    // For a typical 0.4mm nozzle at 0.2mm layer, area ≈ 0.08 mm².
    // Flow rate = area × speed.
    let layer_ratio = match &material.rheology {
        RheologyModel::HerschelBulkley { .. } | RheologyModel::Bingham { .. } => 0.6,
        _ => 0.5,
    };
    let layer_height = nozzle_diameter * layer_ratio;
    let cross_section = nozzle_diameter * layer_height;
    let flow = cross_section * print_speed * 1.05; // 5% overfill for bonding

    let reason = format!(
        "flow={:.3}mm³/s: {:.2} × {:.2}mm cross_section × {:.0}mm/s + 5% overfill",
        flow, nozzle_diameter, layer_height, print_speed,
    );
    (flow, reason)
}

fn estimate_pressure(material: &Material, flow_rate: f64, nozzle_diameter: f64) -> (f64, String) {
    // Simplified pressure estimate using Poiseuille-like scaling.
    // For non-Newtonian, use apparent viscosity at characteristic shear rate.
    let shear_rate_est = if nozzle_diameter > 0.0 {
        (4.0 * flow_rate) / (std::f64::consts::PI * (nozzle_diameter * 0.5).powi(3))
    } else {
        100.0
    };

    let viscosity = crate::viscosity(&material.rheology, shear_rate_est);
    let nozzle_length = 10.0; // mm, typical
    let radius = nozzle_diameter * 0.5; // mm

    // Hagen-Poiseuille (simplified): ΔP = 8μLQ / (πR⁴)
    let pressure_pa = if radius > 0.0 && viscosity > 0.0 {
        8.0 * viscosity * nozzle_length * flow_rate
            / (std::f64::consts::PI * (radius * 1e-3).powi(4))
    } else {
        0.0
    };

    let pressure_kpa = pressure_pa / 1000.0;
    let reason = format!(
        "P≈{:.0}kPa: η_app={:.2}Pa·s @ γ̇≈{:.0}/s, L={:.0}mm, R={:.2}mm",
        pressure_kpa, viscosity, shear_rate_est, nozzle_length, radius,
    );
    (pressure_kpa, reason)
}

fn estimate_temperature(material: &Material) -> Option<f64> {
    // Materials that benefit from temperature control.
    let name_lower = material.name.to_lowercase();
    if name_lower.contains("chocolate") {
        Some(45.0) // chocolate tempering range
    } else if name_lower.contains("gelma") {
        Some(37.0) // physiological temperature for GelMA
    } else if name_lower.contains("dough") || name_lower.contains("paste") {
        Some(25.0) // room temperature
    } else {
        None
    }
}

fn compute_confidence(
    features: &GeometryFeatures,
    material: &Material,
    nozzle: f64,
    _layer_height: f64,
    _speed: f64,
    _flow: f64,
) -> f64 {
    let mut score: f64 = 1.0;

    // Penalise extreme geometries.
    if features.min_feature_size < 0.2 {
        score -= 0.2;
    }
    if features.max_overhang_angle > 70.0 {
        score -= 0.15;
    }
    if features.volume < 1.0 {
        score -= 0.1;
    }

    // Penalise poor nozzle-feature match.
    if nozzle > features.min_feature_size * 0.8 {
        score -= 0.2;
    }

    // Material-specific confidence adjustments.
    let has_good_db_entry = crate::get_material(&material.name).is_some();
    if !has_good_db_entry {
        score -= 0.15;
    }

    score.clamp(0.0, 1.0)
}

fn material_name_short(name: &str) -> &str {
    if name.len() > 12 {
        &name[..12]
    } else {
        name
    }
}

/// Score the overall recommendation quality on multiple axes.
pub fn score_recommendation(
    rec: &PrintRecommendation,
    features: &GeometryFeatures,
) -> ParameterScores {
    // Nozzle appropriateness
    let nozzle_score = if rec.nozzle_diameter <= features.min_feature_size * 0.5 {
        1.0
    } else if rec.nozzle_diameter <= features.min_feature_size * 0.8 {
        0.7
    } else {
        0.3
    };

    // Layer height is reasonable fraction of nozzle
    let ratio = rec.layer_height / rec.nozzle_diameter;
    let layer_score = if (0.2..=0.75).contains(&ratio) {
        1.0
    } else if (0.1..=0.9).contains(&ratio) {
        0.6
    } else {
        0.2
    };

    // Speed is within typical ranges
    let speed_score = if (5.0..=40.0).contains(&rec.print_speed) {
        1.0
    } else if (1.0..=60.0).contains(&rec.print_speed) {
        0.5
    } else {
        0.2
    };

    // Flow rate is reasonable
    let flow_score = if (0.01..=10.0).contains(&rec.flow_rate) {
        1.0
    } else {
        0.4
    };

    let overall = nozzle_score * 0.3 + layer_score * 0.25 + speed_score * 0.25 + flow_score * 0.2;

    ParameterScores {
        nozzle_score,
        layer_score,
        speed_score,
        flow_score,
        overall,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_core::RheologyModel;

    fn sample_material() -> Material {
        Material {
            name: "TestAlginate".to_string(),
            density: 1010.0,
            rheology: RheologyModel::CarreauYasuda {
                eta_zero: 50.0,
                eta_inf: 0.05,
                lambda: 0.3,
                a: 0.4,
                n: 0.4,
            },
            curing: None,
            coaxial: None,
        }
    }

    fn sample_geometry() -> GeometryFeatures {
        GeometryFeatures {
            volume: 1000.0,
            surface_area: 600.0,
            height: 20.0,
            min_feature_size: 1.0,
            max_overhang_angle: 45.0,
            part_count: 1,
        }
    }

    #[test]
    fn test_recommend_returns_valid_params() {
        let material = sample_material();
        let features = sample_geometry();
        let rec = recommend_parameters(&features, &material);

        assert!(rec.nozzle_diameter > 0.1);
        assert!(rec.layer_height > 0.01);
        assert!(rec.print_speed > 0.0);
        assert!(rec.flow_rate > 0.0);
        assert!(rec.confidence >= 0.0);
        assert!(rec.confidence <= 1.0);
        assert!(!rec.reasoning.is_empty());
    }

    #[test]
    fn test_recommend_yield_stress_needs_larger_nozzle() {
        let material = Material {
            name: "Dough".to_string(),
            density: 1300.0,
            rheology: RheologyModel::HerschelBulkley {
                tau_yield: 500.0,
                k: 5.0,
                n: 0.5,
            },
            curing: None,
            coaxial: None,
        };
        let features = sample_geometry();
        let rec = recommend_parameters(&features, &material);

        assert!(rec.nozzle_diameter >= 0.6);
    }

    #[test]
    fn test_recommend_low_viscosity_slow_speed() {
        let material = Material {
            name: "Water".to_string(),
            density: 1000.0,
            rheology: RheologyModel::Newtonian { viscosity: 0.5 },
            curing: None,
            coaxial: None,
        };
        let features = sample_geometry();
        let rec = recommend_parameters(&features, &material);

        assert!(rec.print_speed <= 15.0);
    }

    #[test]
    fn test_geometry_features_extreme_affects_confidence() {
        let material = sample_material();
        let good = recommend_parameters(&sample_geometry(), &material);

        let bad_features = GeometryFeatures {
            volume: 0.5,
            surface_area: 3.0,
            height: 5.0,
            min_feature_size: 0.1,
            max_overhang_angle: 85.0,
            part_count: 1,
        };
        let bad = recommend_parameters(&bad_features, &material);

        assert!(good.confidence >= bad.confidence);
    }

    #[test]
    fn test_score_recommendation_returns_scores() {
        let material = sample_material();
        let features = sample_geometry();
        let rec = recommend_parameters(&features, &material);
        let scores = score_recommendation(&rec, &features);

        assert!(scores.overall >= 0.0);
        assert!(scores.overall <= 1.0);
        assert!(scores.nozzle_score >= 0.0);
    }

    #[test]
    fn test_chocolate_temperature() {
        let material = Material {
            name: "Dark Chocolate".to_string(),
            density: 1260.0,
            rheology: RheologyModel::Bingham {
                tau_yield: 30.0,
                mu_p: 3.0,
            },
            curing: None,
            coaxial: None,
        };
        let features = sample_geometry();
        let rec = recommend_parameters(&features, &material);
        assert_eq!(rec.temperature, Some(45.0));
    }
}
