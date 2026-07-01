// Basic 3-axis slicer for standard thermoplastics
// Licensed under Apache 2.0

use tpt_core::{Profile, InfillPattern};
use tpt_geometry::Mesh;
use nalgebra::Point3;

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

/// Slice result containing layers
#[derive(Debug, Clone)]
pub struct SliceResult {
    pub layers: Vec<SliceLayer>,
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

    /// Slice a mesh into layers
    pub fn slice(&self, mesh: &Mesh) -> SliceResult {
        let mut layers = Vec::new();
        
        // Find Z bounds
        let z_min = mesh.vertices.iter()
            .map(|v| v.z)
            .fold(f64::INFINITY, f64::min);
        let z_max = mesh.vertices.iter()
            .map(|v| v.z)
            .fold(f64::NEG_INFINITY, f64::max);
        
        // Generate layers
        let mut z = z_min;
        while z <= z_max {
            let layer = self.slice_layer(mesh, z);
            if !layer.polygons.is_empty() {
                layers.push(layer);
            }
            z += self.params.layer_height;
        }
        
        SliceResult { layers }
    }

    fn slice_layer(&self, mesh: &Mesh, z: f64) -> SliceLayer {
        // Simplified: just find triangles intersecting this Z plane
        // A real implementation would use proper polygon clipping
        
        let mut polygons = Vec::new();
        
        for tri in &mesh.triangles {
            // Check if triangle intersects this Z plane
            if (tri.v1.z <= z && tri.v2.z >= z) || (tri.v2.z <= z && tri.v3.z >= z) || (tri.v3.z <= z && tri.v1.z >= z) {
                // Intersecting - create a simple polygon
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

    #[test]
    fn test_slicer_creation() {
        let params = SlicingParams {
            layer_height: 0.2,
            infill_density: 0.2,
            infill_pattern: InfillPattern::Grid,
            perimeters: 3,
            top_solid_layers: 3,
            bottom_solid_layers: 3,
        };
        
        let _slicer = Slicer::new(params);
    }
}