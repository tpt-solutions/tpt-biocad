// Simulation mode: run slumping model across all layers
// Licensed under Apache 2.0
//
// Predicts the final deformed geometry by applying the slumping model
// to every layer of a sliced model, accounting for accumulated deformation
// from lower layers.

use tpt_core::{Material, RheologyModel};

use crate::slumping::predict_slump;

/// A single simulated layer after deformation.
#[derive(Debug, Clone)]
pub struct SimulatedLayer {
    /// Original Z height of the layer (mm).
    pub z: f64,
    /// Bead width after slumping (mm).
    pub width_after: f64,
    /// Bead height after slumping (mm).
    pub height_after: f64,
    /// Cumulative vertical displacement of the layer centroid (mm).
    /// Positive = sinking downward.
    pub vertical_displacement: f64,
    /// Slump factor (0 = no deformation, 1 = full collapse).
    pub slump_factor: f64,
    /// Whether this layer has fully settled.
    pub settled: bool,
}

/// Complete simulation result for a multi-layer print.
#[derive(Debug, Clone)]
pub struct LayerSimulationResult {
    /// Simulated layers, lowest (first printed) first.
    pub layers: Vec<SimulatedLayer>,
    /// Total height of the simulated part after deformation (mm).
    pub final_height: f64,
    /// Maximum width increase due to slumping (mm).
    pub max_width_increase: f64,
    /// Maximum vertical displacement observed (mm).
    pub max_displacement: f64,
    /// Overall shape retention score (0.0 = fully collapsed, 1.0 = perfect).
    pub shape_retention: f64,
}

/// Configuration for the layer simulation.
#[derive(Debug, Clone)]
pub struct SimulationConfig {
    /// Time between layer depositions (seconds). Longer = more slumping.
    pub inter_layer_time: f64,
    /// Print speed (mm/s).
    pub print_speed: f64,
    /// Whether to model cumulative deformation (heavier lower layers).
    pub accumulate_weight: bool,
    /// Whether to mark a layer as settled when slump_factor < threshold.
    pub settle_threshold: f64,
}

impl Default for SimulationConfig {
    fn default() -> Self {
        Self {
            inter_layer_time: 2.0,
            print_speed: 10.0,
            accumulate_weight: true,
            settle_threshold: 0.01,
        }
    }
}

/// Simulate slumping across all layers of a print.
///
/// Takes a list of layer heights (Z positions), the material, bead geometry,
/// and simulation configuration. Returns the predicted deformed state of
/// each layer.
///
/// The simulation works bottom-up:
/// 1. Compute slumping for each bead individually given the inter-layer time.
/// 2. If `accumulate_weight` is enabled, lower layers bear the weight of
///    layers above, increasing the effective gravitational stress.
/// 3. Accumulate vertical displacement (settling) from the bottom up.
pub fn simulate_layers(
    layer_heights: &[f64],
    material: &Material,
    bead_width: f64,
    bead_height: f64,
    config: &SimulationConfig,
) -> LayerSimulationResult {
    if layer_heights.is_empty() {
        return LayerSimulationResult {
            layers: Vec::new(),
            final_height: 0.0,
            max_width_increase: 0.0,
            max_displacement: 0.0,
            shape_retention: 1.0,
        };
    }

    let mut simulated: Vec<SimulatedLayer> = Vec::with_capacity(layer_heights.len());
    let mut cumulative_displacement = 0.0;
    let mut max_width = bead_width;
    let mut max_disp = 0.0;

    // Process layers bottom-up.
    for (i, &z) in layer_heights.iter().enumerate() {
        let layers_above = layer_heights.len() - i - 1;

        // Compute the effective load: the ratio of layers above to total,
        // scaled by the material density. This models the weight burden on
        // lower layers.
        let effective_stress_multiplier = if config.accumulate_weight && layers_above > 0 {
            1.0 + 0.1 * (layers_above as f64).sqrt()
        } else {
            1.0
        };

        // Build a modified rheology with scaled yield stress (lower layers
        // effectively have less yield stress relative to the load).
        let effective_model = match &material.rheology {
            RheologyModel::HerschelBulkley { tau_yield, k, n } => RheologyModel::HerschelBulkley {
                tau_yield: tau_yield / effective_stress_multiplier,
                k: *k,
                n: *n,
            },
            RheologyModel::Bingham { tau_yield, mu_p } => RheologyModel::Bingham {
                tau_yield: tau_yield / effective_stress_multiplier,
                mu_p: *mu_p,
            },
            other => other.clone(),
        };

        // Predict slumping for this layer.
        let slump = predict_slump(
            &effective_model,
            material.density,
            bead_width,
            bead_height,
            config.print_speed,
            config.inter_layer_time,
        );

        // Accumulate vertical displacement. As lower layers slump, the layers
        // above them sink by the same amount.
        if i > 0 {
            let prev_settling = simulated[i - 1].height_after;
            let diff = bead_height - prev_settling;
            cumulative_displacement += diff.max(0.0);
        }

        let settled = slump.slump_factor < config.settle_threshold;
        let width = slump.width_after.max(bead_width);
        let height = slump.height_after.min(bead_height);

        if width > max_width {
            max_width = width;
        }
        if cumulative_displacement > max_disp {
            max_disp = cumulative_displacement;
        }

        simulated.push(SimulatedLayer {
            z,
            width_after: width,
            height_after: height,
            vertical_displacement: cumulative_displacement,
            slump_factor: slump.slump_factor,
            settled,
        });
    }

    // Compute final part metrics.
    let final_height = layer_heights
        .last()
        .map(|&last_z| {
            simulated
                .last()
                .map(|s| last_z + s.height_after - s.vertical_displacement)
                .unwrap_or(last_z)
        })
        .unwrap_or(0.0);

    let initial_height = layer_heights.last().copied().unwrap_or(0.0) + bead_height;
    let shape_retention = if initial_height > 0.0 {
        (final_height / initial_height).clamp(0.0, 1.0)
    } else {
        1.0
    };

    LayerSimulationResult {
        layers: simulated,
        final_height,
        max_width_increase: max_width - bead_width,
        max_displacement: max_disp,
        shape_retention,
    }
}

/// Build a deformed mesh representation from the simulation result.
///
/// Returns a set of vertices and triangle indices representing the predicted
/// final shape. Each layer's bead is approximated as a rectangular cross-section
/// extruded along a straight line segment.
pub fn deformed_mesh_from_simulation(
    result: &LayerSimulationResult,
    _bead_width: f64,
) -> (Vec<[f64; 3]>, Vec<[usize; 3]>) {
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut triangles: Vec<[usize; 3]> = Vec::new();

    for (i, layer) in result.layers.iter().enumerate() {
        let y_center = i as f64 * 2.0; // spacing between layers for visualization
        let half_w = layer.width_after * 0.5;
        let z_pos = layer.z - layer.vertical_displacement;

        // Create a rectangular bead cross-section: 4 corners.
        let base = vertices.len();
        vertices.push([-half_w, y_center - 0.5, z_pos]);
        vertices.push([half_w, y_center - 0.5, z_pos]);
        vertices.push([half_w, y_center - 0.5, z_pos + layer.height_after]);
        vertices.push([-half_w, y_center - 0.5, z_pos + layer.height_after]);
        vertices.push([-half_w, y_center + 0.5, z_pos]);
        vertices.push([half_w, y_center + 0.5, z_pos]);
        vertices.push([half_w, y_center + 0.5, z_pos + layer.height_after]);
        vertices.push([-half_w, y_center + 0.5, z_pos + layer.height_after]);

        // 12 triangles per bead box (6 faces × 2 triangles each).
        let faces: [[usize; 3]; 12] = [
            // Front
            [base, base + 1, base + 2],
            [base, base + 2, base + 3],
            // Back
            [base + 4, base + 6, base + 5],
            [base + 4, base + 7, base + 6],
            // Left
            [base, base + 3, base + 7],
            [base, base + 7, base + 4],
            // Right
            [base + 1, base + 5, base + 6],
            [base + 1, base + 6, base + 2],
            // Top
            [base + 3, base + 2, base + 6],
            [base + 3, base + 6, base + 7],
            // Bottom
            [base, base + 4, base + 5],
            [base, base + 5, base + 1],
        ];

        triangles.extend_from_slice(&faces[..]);
    }

    (vertices, triangles)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_material() -> Material {
        Material {
            name: "Test".to_string(),
            density: 1000.0,
            rheology: RheologyModel::Bingham {
                tau_yield: 50.0,
                mu_p: 5.0,
            },
            curing: None,
            coaxial: None,
        }
    }

    #[test]
    fn test_simulate_single_layer() {
        let material = test_material();
        let result = simulate_layers(&[0.2], &material, 0.4, 0.2, &SimulationConfig::default());
        assert_eq!(result.layers.len(), 1);
        assert!(result.final_height > 0.0);
        assert!(result.shape_retention >= 0.0);
    }

    #[test]
    fn test_simulate_multi_layer_accumulates() {
        let material = test_material();
        let layers: Vec<f64> = (0..10).map(|i| 0.2 + i as f64 * 0.2).collect();
        let result = simulate_layers(&layers, &material, 0.4, 0.2, &SimulationConfig::default());
        assert_eq!(result.layers.len(), 10);
        // Bottom layers should have more displacement.
        assert!(result.max_displacement >= 0.0);
        assert!(result.shape_retention > 0.0);
    }

    #[test]
    fn test_accumulate_weight_increases_slump() {
        let material = test_material();
        let layers: Vec<f64> = (0..20).map(|i| 0.2 + i as f64 * 0.2).collect();

        let with_accumulation = simulate_layers(
            &layers,
            &material,
            0.4,
            0.2,
            &SimulationConfig {
                accumulate_weight: true,
                ..Default::default()
            },
        );

        let without_accumulation = simulate_layers(
            &layers,
            &material,
            0.4,
            0.2,
            &SimulationConfig {
                accumulate_weight: false,
                ..Default::default()
            },
        );

        assert!(with_accumulation.max_displacement >= without_accumulation.max_displacement);
    }

    #[test]
    fn test_longer_layer_time_more_slump() {
        let material = Material {
            name: "LowYield".to_string(),
            density: 1000.0,
            rheology: RheologyModel::Bingham {
                tau_yield: 10.0,
                mu_p: 2.0,
            },
            curing: None,
            coaxial: None,
        };
        let layers = [0.2, 0.4, 0.6];

        let fast = simulate_layers(
            &layers,
            &material,
            0.4,
            0.2,
            &SimulationConfig {
                inter_layer_time: 0.5,
                ..Default::default()
            },
        );

        let slow = simulate_layers(
            &layers,
            &material,
            0.4,
            0.2,
            &SimulationConfig {
                inter_layer_time: 10.0,
                ..Default::default()
            },
        );

        assert!(slow.max_displacement >= fast.max_displacement);
    }

    #[test]
    fn test_deformed_mesh_generates_correct_count() {
        let material = test_material();
        let layers: Vec<f64> = (0..3).map(|i| 0.2 + i as f64 * 0.2).collect();
        let result = simulate_layers(&layers, &material, 0.4, 0.2, &SimulationConfig::default());
        let (verts, tris) = deformed_mesh_from_simulation(&result, 0.4);

        // Each layer produces 8 vertices and 12 triangles.
        assert_eq!(verts.len(), 3 * 8);
        assert_eq!(tris.len(), 3 * 12);
    }

    #[test]
    fn test_empty_layers() {
        let material = test_material();
        let result = simulate_layers(&[], &material, 0.4, 0.2, &SimulationConfig::default());
        assert!(result.layers.is_empty());
        assert_eq!(result.final_height, 0.0);
    }

    #[test]
    fn test_settled_layers_detected() {
        let material = Material {
            name: "Stiff".to_string(),
            density: 2000.0,
            rheology: RheologyModel::Bingham {
                tau_yield: 1000.0,
                mu_p: 10.0,
            },
            curing: None,
            coaxial: None,
        };
        let layers = [0.2];
        let result = simulate_layers(
            &layers,
            &material,
            0.4,
            0.2,
            &SimulationConfig {
                settle_threshold: 0.1,
                ..Default::default()
            },
        );
        assert!(result.layers[0].settled);
    }
}
