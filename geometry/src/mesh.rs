// Mesh data structures
// Licensed under Apache 2.0

use nalgebra::Point3;

/// Triangle face in a mesh
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub v1: Point3<f64>,
    pub v2: Point3<f64>,
    pub v3: Point3<f64>,
    pub normal: Point3<f64>,
}

/// Mesh representation
#[derive(Debug, Clone, Default)]
pub struct Mesh {
    pub vertices: Vec<Point3<f64>>,
    pub triangles: Vec<Triangle>,
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    /// Calculate mesh volume using divergence theorem
    pub fn volume(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                // Signed volume of tetrahedron formed by triangle and origin
                (t.v1.x * (t.v2.y * t.v3.z - t.v3.y * t.v2.z)
                    - t.v1.y * (t.v2.x * t.v3.z - t.v3.x * t.v2.z)
                    + t.v1.z * (t.v2.x * t.v3.y - t.v3.x * t.v2.y))
                    / 6.0
            })
            .sum()
    }

    /// Calculate mesh surface area
    pub fn surface_area(&self) -> f64 {
        self.triangles
            .iter()
            .map(|t| {
                // Area using cross product
                let e1 = t.v2 - t.v1;
                let e2 = t.v3 - t.v1;
                e1.cross(&e2).norm() / 2.0
            })
            .sum()
    }
}
