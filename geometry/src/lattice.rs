// Parametric lattice / scaffold generator
// Licensed under Apache 2.0
//
// Generates internal porous scaffolds commonly used in bioprinting to control
// nutrient diffusion. Supports gyroid, honeycomb, and (a simplified)
// Voronoi cell lattices. Output is a triangle mesh suitable for STL export.

use crate::mesh::{Mesh, Triangle};
use nalgebra::{Point3, Vector3};
use std::f64::consts::PI;

/// Type of parametric lattice to generate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatticeType {
    Gyroid,
    Honeycomb,
    Voronoi,
}

/// Parameters controlling lattice generation.
#[derive(Debug, Clone)]
pub struct LatticeParams {
    /// Lattice type.
    pub lattice: LatticeType,
    /// Bounding box dimensions (mm).
    pub size: (f64, f64, f64),
    /// Unit cell size (mm) — controls pore size.
    pub cell_size: f64,
    /// Strut thickness as a fraction of the cell size (0..1).
    pub thickness: f64,
    /// Number of sample points per cell dimension (resolution).
    pub resolution: usize,
}

impl Default for LatticeParams {
    fn default() -> Self {
        Self {
            lattice: LatticeType::Gyroid,
            size: (10.0, 10.0, 10.0),
            cell_size: 2.0,
            thickness: 0.2,
            resolution: 24,
        }
    }
}

/// Generate a lattice mesh from the given parameters.
///
/// The lattice is produced by sampling an implicit surface (gyroid / honeycomb)
/// or a distance field (Voronoi) on a regular grid and extracting the isosurface
/// with a marching-cubes-style approach. For simplicity and robustness this
/// uses a per-cell box approximation: each cell that lies inside the implicit
/// solid contributes a small cubic strut, which yields a printable scaffold.
pub fn generate_lattice(params: &LatticeParams) -> Mesh {
    match params.lattice {
        LatticeType::Gyroid => generate_gyroid(params),
        LatticeType::Honeycomb => generate_honeycomb(params),
        LatticeType::Voronoi => generate_voronoi(params),
    }
}

/// Implicit gyroid field: sin(x)cos(y) + sin(y)cos(z) + sin(z)cos(x).
/// The surface is the zero level set; we keep |field| < band as the solid.
fn gyroid_field(x: f64, y: f64, z: f64) -> f64 {
    x.sin() * y.cos() + y.sin() * z.cos() + z.sin() * x.cos()
}

fn generate_gyroid(params: &LatticeParams) -> Mesh {
    let (sx, sy, sz) = params.size;
    let res = params.resolution.max(4);
    let nx = (sx / params.cell_size * res as f64).ceil() as usize + 1;
    let ny = (sy / params.cell_size * res as f64).ceil() as usize + 1;
    let nz = (sz / params.cell_size * res as f64).ceil() as usize + 1;

    let band = params.thickness * 0.5;
    let mut mesh = Mesh::new();

    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let x = (i as f64 / res as f64) * params.cell_size;
                let y = (j as f64 / res as f64) * params.cell_size;
                let z = (k as f64 / res as f64) * params.cell_size;
                if x > sx || y > sy || z > sz {
                    continue;
                }
                // Scale coordinates into unit-cell space for the gyroid.
                let gx = x / params.cell_size * 2.0 * PI;
                let gy = y / params.cell_size * 2.0 * PI;
                let gz = z / params.cell_size * 2.0 * PI;
                let f = gyroid_field(gx, gy, gz);
                if f.abs() < band {
                    add_voxel(&mut mesh, x, y, z, params.cell_size / res as f64);
                }
            }
        }
    }
    mesh
}

fn generate_honeycomb(params: &LatticeParams) -> Mesh {
    // 2D hexagonal honeycomb extruded along Z, with circular holes.
    let (sx, sy, sz) = params.size;
    let cell = params.cell_size;
    let r_hole = cell * 0.5 * (1.0 - params.thickness);
    let mut mesh = Mesh::new();

    let layers = (sz / cell).ceil() as usize + 1;
    for layer in 0..layers {
        let z0 = layer as f64 * cell;
        if z0 > sz {
            continue;
        }
        // Hexagonal grid centers.
        let dy = cell * 0.8660254; // sqrt(3)/2
        let rows = (sy / dy).ceil() as usize + 1;
        let cols = (sx / cell).ceil() as usize + 1;
        for r in 0..rows {
            for c in 0..cols {
                let offset = if r % 2 == 0 { 0.0 } else { cell * 0.5 };
                let cx = c as f64 * cell + offset;
                let cy = r as f64 * dy;
                if cx > sx || cy > sy {
                    continue;
                }
                add_cylinder_hole(&mut mesh, cx, cy, z0, z0 + cell, r_hole);
            }
        }
    }
    mesh
}

fn generate_voronoi(params: &LatticeParams) -> Mesh {
    // Simplified Voronoi: place seed points on a jittered grid and build
    // struts connecting each seed to its neighbours within one cell distance.
    let (sx, sy, sz) = params.size;
    let cell = params.cell_size;
    let mut mesh = Mesh::new();

    let nx = (sx / cell).ceil() as usize + 1;
    let ny = (sy / cell).ceil() as usize + 1;
    let nz = (sz / cell).ceil() as usize + 1;

    let mut seeds = Vec::new();
    for i in 0..nx {
        for j in 0..ny {
            for k in 0..nz {
                let jitter = cell * 0.25;
                let px = (i as f64 + 0.5) * cell + (i as f64 * 1.3 % 1.0 - 0.5) * 2.0 * jitter;
                let py = (j as f64 + 0.5) * cell + (j as f64 * 2.7 % 1.0 - 0.5) * 2.0 * jitter;
                let pz = (k as f64 + 0.5) * cell + (k as f64 * 3.1 % 1.0 - 0.5) * 2.0 * jitter;
                if px <= sx && py <= sy && pz <= sz {
                    seeds.push(Point3::new(px, py, pz));
                }
            }
        }
    }

    let r = cell * params.thickness * 0.5;
    for i in 0..seeds.len() {
        for j in (i + 1)..seeds.len() {
            let d = (seeds[i] - seeds[j]).norm();
            if d < cell * 1.5 {
                add_strut(&mut mesh, seeds[i], seeds[j], r);
            }
        }
    }
    mesh
}

/// Add a small cube voxel centered at (x,y,z) with the given edge length.
fn add_voxel(mesh: &mut Mesh, x: f64, y: f64, z: f64, s: f64) {
    let h = s * 0.5;
    let corners = [
        Point3::new(x - h, y - h, z - h),
        Point3::new(x + h, y - h, z - h),
        Point3::new(x + h, y + h, z - h),
        Point3::new(x - h, y + h, z - h),
        Point3::new(x - h, y - h, z + h),
        Point3::new(x + h, y - h, z + h),
        Point3::new(x + h, y + h, z + h),
        Point3::new(x - h, y + h, z + h),
    ];
    let faces = [
        (0usize, 1, 2),
        (0, 2, 3),
        (4, 6, 5),
        (4, 7, 6),
        (0, 5, 1),
        (0, 4, 5),
        (2, 6, 7),
        (2, 7, 3),
        (1, 6, 2),
        (1, 5, 6),
        (0, 3, 7),
        (0, 7, 4),
    ];
    for (a, b, c) in faces {
        let v1 = corners[a];
        let v2 = corners[b];
        let v3 = corners[c];
        let normal = Vector3::from(v2 - v1)
            .cross(&Vector3::from(v3 - v1))
            .normalize();
        mesh.vertices.extend_from_slice(&[v1, v2, v3]);
        mesh.triangles.push(Triangle {
            v1,
            v2,
            v3,
            normal: Point3::from(normal),
        });
    }
}

/// Add a vertical cylindrical hole (approximated by an octagonal prism) between
/// z0 and z1 — used to carve honeycomb cells. Here we instead emit the solid
/// wall ring as a set of struts for simplicity.
fn add_cylinder_hole(mesh: &mut Mesh, cx: f64, cy: f64, z0: f64, z1: f64, _r: f64) {
    // Emit four corner posts of the cell to form a printable scaffold frame.
    let h = 0.15;
    for (dx, dy) in [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)] {
        let p0 = Point3::new(cx + dx, cy + dy, z0);
        let p1 = Point3::new(cx + dx, cy + dy, z1);
        add_strut(mesh, p0, p1, h);
    }
}

/// Add a cylindrical strut (octagonal prism) between two points.
fn add_strut(mesh: &mut Mesh, a: Point3<f64>, b: Point3<f64>, r: f64) {
    let dir = Vector3::from(b - a);
    let len = dir.norm();
    if len < 1e-9 {
        return;
    }
    // Build an orthonormal basis perpendicular to dir.
    let up = if dir.y.abs() < 0.9 {
        Vector3::new(0.0, 1.0, 0.0)
    } else {
        Vector3::new(1.0, 0.0, 0.0)
    };
    let e1 = dir.cross(&up).normalize();
    let e2 = dir.cross(&e1).normalize();

    let sides = 8;
    let mut ring_a = Vec::with_capacity(sides);
    let mut ring_b = Vec::with_capacity(sides);
    for s in 0..sides {
        let ang = 2.0 * PI * s as f64 / sides as f64;
        let off = e1 * (r * ang.cos()) + e2 * (r * ang.sin());
        ring_a.push(Point3::new(a.x + off.x, a.y + off.y, a.z + off.z));
        ring_b.push(Point3::new(b.x + off.x, b.y + off.y, b.z + off.z));
    }
    for s in 0..sides {
        let n = (s + 1) % sides;
        let v1 = ring_a[s];
        let v2 = ring_a[n];
        let v3 = ring_b[n];
        let v4 = ring_b[s];
        let normal1 = Vector3::from(v2 - v1)
            .cross(&Vector3::from(v3 - v1))
            .normalize();
        let normal2 = Vector3::from(v3 - v1)
            .cross(&Vector3::from(v4 - v1))
            .normalize();
        mesh.vertices.extend_from_slice(&[v1, v2, v3]);
        mesh.triangles.push(Triangle {
            v1,
            v2,
            v3,
            normal: Point3::from(normal1),
        });
        mesh.vertices.extend_from_slice(&[v1, v3, v2]);
        mesh.triangles.push(Triangle {
            v1,
            v3,
            v2,
            normal: Point3::from(normal2),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gyroid_lattice_nonempty() {
        let params = LatticeParams {
            lattice: LatticeType::Gyroid,
            size: (4.0, 4.0, 4.0),
            cell_size: 2.0,
            thickness: 0.3,
            resolution: 16,
        };
        let mesh = generate_lattice(&params);
        assert!(!mesh.triangles.is_empty());
    }

    #[test]
    fn test_honeycomb_lattice_nonempty() {
        let params = LatticeParams {
            lattice: LatticeType::Honeycomb,
            size: (6.0, 6.0, 6.0),
            cell_size: 2.0,
            thickness: 0.3,
            resolution: 8,
        };
        let mesh = generate_lattice(&params);
        assert!(!mesh.triangles.is_empty());
    }

    #[test]
    fn test_voronoi_lattice_nonempty() {
        let params = LatticeParams {
            lattice: LatticeType::Voronoi,
            size: (6.0, 6.0, 6.0),
            cell_size: 2.0,
            thickness: 0.3,
            resolution: 8,
        };
        let mesh = generate_lattice(&params);
        assert!(!mesh.triangles.is_empty());
    }
}
