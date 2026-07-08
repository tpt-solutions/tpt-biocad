// Basic 3-axis slicer for standard thermoplastics
// Licensed under Apache 2.0

use nalgebra::Point3;
use tpt_core::{InfillPattern, Material};
use tpt_geometry::Mesh;

/// Slicing parameters
#[derive(Debug, Clone)]
pub struct SlicingParams {
    pub layer_height: f64,
    pub infill_density: f64,
    pub infill_pattern: InfillPattern,
    pub perimeters: usize,
    pub top_solid_layers: usize,
    pub bottom_solid_layers: usize,
}

/// A command in the generated toolpath.
#[derive(Debug, Clone)]
pub enum ToolpathCommand {
    /// Move to a position (G0 rapid or G1 linear).
    Move {
        x: f64,
        y: f64,
        z: f64,
        e: Option<f64>,
        f: Option<f64>,
        rapid: bool,
    },
    /// Set temperature (M104/M109).
    SetTemperature { temp: f64, wait: bool },
    /// Pneumatic pressure control (M300).
    PneumaticPressure { pressure: f64, duration: f64 },
    /// UV curing (M302) — inserted between layers for photo-crosslinkable materials.
    UVCuring { intensity: f64, duration: f64 },
    /// Coaxial cross-linker flow (M303).
    CoaxialFlow,
}

/// Slice result containing toolpath commands
#[derive(Debug, Clone)]
pub struct SliceResult {
    pub layers: Vec<SliceLayer>,
    pub commands: Vec<ToolpathCommand>,
}

/// Single layer of sliced geometry
#[derive(Debug, Clone)]
pub struct SliceLayer {
    pub z: f64,
    pub polygons: Vec<Polygon>,
}

/// Polygon for toolpath
#[derive(Debug, Clone)]
pub struct Polygon {
    pub points: Vec<Point3<f64>>,
    pub is_perimeter: bool,
}

/// Basic slicer implementation
pub struct Slicer {
    params: SlicingParams,
}

impl Slicer {
    pub fn new(params: SlicingParams) -> Self {
        Self { params }
    }

    /// Slice a mesh into layers and generate toolpath commands.
    /// If a material is provided, UV curing and coaxial commands are inserted
    /// as needed.
    pub fn slice(&self, mesh: &Mesh, material: Option<&Material>) -> SliceResult {
        let mut layers = Vec::new();
        let mut commands = Vec::new();

        // Find Z bounds
        let z_min = mesh
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::INFINITY, f64::min);
        let z_max = mesh
            .vertices
            .iter()
            .map(|v| v.z)
            .fold(f64::NEG_INFINITY, f64::max);

        // Generate layers
        let mut z = z_min;
        while z <= z_max {
            let layer = self.slice_layer(mesh, z);
            if !layer.polygons.is_empty() {
                layers.push(layer);

                // Generate toolpath commands for this layer
                self.generate_layer_commands(&mut commands, z, material);
            }
            z += self.params.layer_height;
        }

        SliceResult { layers, commands }
    }

    fn generate_layer_commands(
        &self,
        commands: &mut Vec<ToolpathCommand>,
        z: f64,
        material: Option<&Material>,
    ) {
        // Move to layer height
        commands.push(ToolpathCommand::Move {
            x: 0.0,
            y: 0.0,
            z,
            e: None,
            f: None,
            rapid: true,
        });

        // UV curing: insert M302 between layers for photo-crosslinkable materials
        if let Some(mat) = material {
            if let Some(ref curing) = mat.curing {
                commands.push(ToolpathCommand::UVCuring {
                    intensity: curing.uv_intensity,
                    duration: curing.exposure_time,
                });
            }

            // Coaxial: insert M303 if material uses dual-channel extrusion
            if mat.coaxial.is_some() {
                commands.push(ToolpathCommand::CoaxialFlow);
            }
        }
    }

    fn slice_layer(&self, mesh: &Mesh, z: f64) -> SliceLayer {
        let mut polygons = Vec::new();

        for tri in &mesh.triangles {
            if (tri.v1.z <= z && tri.v2.z >= z)
                || (tri.v2.z <= z && tri.v3.z >= z)
                || (tri.v3.z <= z && tri.v1.z >= z)
            {
                let points = vec![tri.v1, tri.v2, tri.v3];
                polygons.push(Polygon {
                    points,
                    is_perimeter: true,
                });
            }
        }

        SliceLayer { z, polygons }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_core::RheologyModel;

    fn test_mesh() -> Mesh {
        let mut mesh = Mesh::new();
        let v = |x, y, z| nalgebra::Point3::new(x, y, z);
        mesh.vertices = vec![
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(0.5, 1.0, 0.0),
            v(0.5, 0.5, 1.0),
        ];
        mesh.triangles = vec![
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(1.0, 0.0, 0.0),
                v3: v(0.5, 0.5, 1.0),
                normal: v(0.0, 0.0, 1.0),
            },
            tpt_geometry::Triangle {
                v1: v(0.0, 0.0, 0.0),
                v2: v(0.5, 1.0, 0.0),
                v3: v(0.5, 0.5, 1.0),
                normal: v(0.0, 0.0, 1.0),
            },
            tpt_geometry::Triangle {
                v1: v(1.0, 0.0, 0.0),
                v2: v(0.5, 1.0, 0.0),
                v3: v(0.5, 0.5, 1.0),
                normal: v(0.0, 0.0, 1.0),
            },
        ];
        mesh
    }

    fn params() -> SlicingParams {
        SlicingParams {
            layer_height: 0.2,
            infill_density: 0.2,
            infill_pattern: InfillPattern::Grid,
            perimeters: 3,
            top_solid_layers: 3,
            bottom_solid_layers: 3,
        }
    }

    #[test]
    fn test_slicer_no_material() {
        let slicer = Slicer::new(params());
        let result = slicer.slice(&test_mesh(), None);
        assert!(!result.layers.is_empty());
        // No UV curing commands without material
        assert!(!result
            .commands
            .iter()
            .any(|c| matches!(c, ToolpathCommand::UVCuring { .. })));
    }

    #[test]
    fn test_slicer_uv_curing() {
        let slicer = Slicer::new(params());
        let material = tpt_core::Material {
            name: "GelMA 10%".to_string(),
            density: 1020.0,
            rheology: RheologyModel::CarreauYasuda {
                eta_zero: 50.0,
                eta_inf: 0.05,
                lambda: 0.3,
                a: 0.4,
                n: 0.4,
            },
            curing: Some(tpt_core::CuringParams {
                uv_intensity: 10.0,
                exposure_time: 30.0,
                wavelength: 365.0,
            }),
            coaxial: None,
        };
        let result = slicer.slice(&test_mesh(), Some(&material));
        assert!(!result.layers.is_empty());
        let uv_cmds: Vec<_> = result
            .commands
            .iter()
            .filter(|c| matches!(c, ToolpathCommand::UVCuring { .. }))
            .collect();
        assert_eq!(uv_cmds.len(), result.layers.len());
    }

    #[test]
    fn test_slicer_coaxial() {
        let slicer = Slicer::new(params());
        let material = tpt_core::Material {
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
            coaxial: Some(tpt_core::CoaxialParams {
                crosslinker_name: "CaCl₂".to_string(),
                flow_ratio: 0.1,
                concentration: 0.1,
            }),
        };
        let result = slicer.slice(&test_mesh(), Some(&material));
        let coax_cmds: Vec<_> = result
            .commands
            .iter()
            .filter(|c| matches!(c, ToolpathCommand::CoaxialFlow))
            .collect();
        assert_eq!(coax_cmds.len(), result.layers.len());
    }
}
