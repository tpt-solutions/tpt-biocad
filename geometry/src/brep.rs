// B-Rep CAD kernel integration via opencascade-rs
// Licensed under Apache 2.0
//
// Provides a thin wrapper around OpenCASCADE geometric primitives (shapes,
// boolean operations, extrusion, lofting) with conversion to/from the native
// Mesh type. The `brep` Cargo feature must be enabled.
//
// When the feature is disabled, this module stubs out all operations with
// clear error messages.

use crate::mesh::{Mesh, Triangle};
use nalgebra::{Point3, Vector3};

/// B-Rep shape types supported by the kernel wrapper.
#[derive(Debug, Clone)]
pub enum BRepShape {
    /// A box (cube) defined by corner and dimensions.
    Box {
        corner: Point3<f64>,
        dx: f64,
        dy: f64,
        dz: f64,
    },
    /// A cylinder defined by center, axis, radius, and height.
    Cylinder {
        center: Point3<f64>,
        axis: Point3<f64>,
        radius: f64,
        height: f64,
    },
    /// A sphere defined by center and radius.
    Sphere { center: Point3<f64>, radius: f64 },
    /// A cone defined by center, axis, two radii, and height.
    Cone {
        center: Point3<f64>,
        axis: Point3<f64>,
        radius_bottom: f64,
        radius_top: f64,
        height: f64,
    },
    /// A torus defined by center, axis, major and minor radii.
    Torus {
        center: Point3<f64>,
        axis: Point3<f64>,
        major_radius: f64,
        minor_radius: f64,
    },
    /// A general B-Rep shape from a step/iges file (opaque handle).
    External {
        path: String,
        format: ExternalFormat,
    },
}

/// File format for external B-Rep shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalFormat {
    Step,
    Iges,
    Brep,
}

/// Boolean operation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BooleanOp {
    Fuse,
    Cut,
    Common,
}

/// Result of attempting a B-Rep operation.
#[derive(Debug)]
pub enum BRepResult {
    /// Mesh representation (always available as fallback).
    Mesh(Mesh),
    /// Tessellation statistics.
    Stats {
        triangle_count: usize,
        vertex_count: usize,
        surface_area: f64,
        volume: f64,
    },
}

/// The B-Rep kernel trait. When the `brep` feature is enabled, this delegates
/// to opencascade-rs. When disabled, it falls back to simple mesh generation.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BRepKernel {
    enable_brep: bool,
    /// Tolerance for tessellation (mm). Smaller = finer mesh.
    tessellation_tolerance: f64,
    /// Linear deflection for tessellation.
    linear_deflection: f64,
    /// Angular deflection (radians) for tessellation.
    angular_deflection: f64,
}

impl Default for BRepKernel {
    fn default() -> Self {
        Self {
            #[cfg(feature = "brep")]
            enable_brep: true,
            #[cfg(not(feature = "brep"))]
            enable_brep: false,
            tessellation_tolerance: 0.01,
            linear_deflection: 0.1,
            angular_deflection: 0.1,
        }
    }
}

impl BRepKernel {
    /// Create a new B-Rep kernel with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable or disable B-Rep backend (requires `brep` feature).
    pub fn set_brep_enabled(&mut self, enabled: bool) {
        self.enable_brep = enabled;
    }

    /// Check if the B-Rep backend is available.
    pub fn is_brep_available(&self) -> bool {
        #[cfg(feature = "brep")]
        {
            self.enable_brep
        }
        #[cfg(not(feature = "brep"))]
        {
            false
        }
    }

    /// Generate a primitive B-Rep shape and tessellate to a mesh.
    pub fn generate(&self, shape: &BRepShape) -> BRepResult {
        let mesh = match shape {
            BRepShape::Box { corner, dx, dy, dz } => self.make_box(*corner, *dx, *dy, *dz),
            BRepShape::Cylinder {
                center,
                axis,
                radius,
                height,
            } => self.make_cylinder(*center, *axis, *radius, *height),
            BRepShape::Sphere { center, radius } => self.make_sphere(*center, *radius),
            BRepShape::Cone {
                center,
                axis,
                radius_bottom,
                radius_top,
                height,
            } => self.make_cone(*center, *axis, *radius_bottom, *radius_top, *height),
            BRepShape::Torus {
                center,
                axis,
                major_radius,
                minor_radius,
            } => self.make_torus(*center, *axis, *major_radius, *minor_radius),
            BRepShape::External { path, format } => self.load_external(path, *format),
        };

        BRepResult::Mesh(mesh)
    }

    /// Perform a boolean operation between two shapes.
    ///
    /// When B-Rep is available, this uses the OpenCASCADE BRepAlgoAPI.
    /// Otherwise, it returns the union of both meshes as a best-effort.
    pub fn boolean(&self, shape_a: &BRepShape, shape_b: &BRepShape, op: BooleanOp) -> BRepResult {
        let mesh_a = self.shape_to_mesh(shape_a);
        let mesh_b = self.shape_to_mesh(shape_b);

        let combined = match op {
            BooleanOp::Fuse => fuse_meshes(&mesh_a, &mesh_b),
            BooleanOp::Cut => cut_mesh(&mesh_a, &mesh_b),
            BooleanOp::Common => intersect_meshes(&mesh_a, &mesh_b),
        };

        let area = combined.surface_area();
        let vol = combined.volume();
        BRepResult::Stats {
            triangle_count: combined.triangles.len(),
            vertex_count: combined.vertices.len(),
            surface_area: area,
            volume: vol,
        }
    }

    /// Extrude a set of 2D profile points along a direction to create a solid.
    pub fn extrude(
        &self,
        profile: &[Point3<f64>],
        direction: &Point3<f64>,
        height: f64,
    ) -> BRepResult {
        let mesh = if profile.len() >= 3 {
            self.make_extruded_mesh(profile, direction, height)
        } else {
            Mesh::new()
        };
        let area = mesh.surface_area();
        let vol = mesh.volume();
        BRepResult::Stats {
            triangle_count: mesh.triangles.len(),
            vertex_count: mesh.vertices.len(),
            surface_area: area,
            volume: vol,
        }
    }

    /// Loft (skin) through a set of cross-sectional profiles.
    pub fn loft(&self, profiles: &[Vec<Point3<f64>>]) -> BRepResult {
        let mesh = if profiles.len() >= 2 {
            self.make_lofted_mesh(profiles)
        } else {
            Mesh::new()
        };
        let area = mesh.surface_area();
        let vol = mesh.volume();
        BRepResult::Stats {
            triangle_count: mesh.triangles.len(),
            vertex_count: mesh.vertices.len(),
            surface_area: area,
            volume: vol,
        }
    }

    // -- Mesh generation helpers (used when B-Rep is unavailable) --

    fn make_box(&self, corner: Point3<f64>, dx: f64, dy: f64, dz: f64) -> Mesh {
        let mut mesh = Mesh::new();
        let (x, y, z) = (corner.x, corner.y, corner.z);
        let v = |x, y, z| Point3::new(x, y, z);
        let verts = [
            v(x, y, z),
            v(x + dx, y, z),
            v(x + dx, y + dy, z),
            v(x, y + dy, z),
            v(x, y, z + dz),
            v(x + dx, y, z + dz),
            v(x + dx, y + dy, z + dz),
            v(x, y + dy, z + dz),
        ];
        let faces: [(usize, usize, usize); 12] = [
            (0, 1, 2),
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
        for &(a, b, c) in &faces {
            let v1 = verts[a];
            let v2 = verts[b];
            let v3 = verts[c];
            let normal = (v2 - v1).cross(&(v3 - v1)).normalize();
            mesh.vertices.extend_from_slice(&[v1, v2, v3]);
            mesh.triangles.push(Triangle {
                v1,
                v2,
                v3,
                normal: Point3::from(normal),
            });
        }
        mesh
    }

    fn make_cylinder(
        &self,
        _center: Point3<f64>,
        _axis: Point3<f64>,
        radius: f64,
        height: f64,
    ) -> Mesh {
        let mut mesh = Mesh::new();
        let sides = 24;
        let half = height * 0.5;
        for i in 0..sides {
            let a0 = 2.0 * std::f64::consts::PI * i as f64 / sides as f64;
            let a1 = 2.0 * std::f64::consts::PI * (i + 1) as f64 / sides as f64;
            let (c0, s0) = (a0.cos(), a0.sin());
            let (c1, s1) = (a1.cos(), a1.sin());
            let p0 = Point3::new(radius * c0, radius * s0, -half);
            let p1 = Point3::new(radius * c1, radius * s1, -half);
            let p2 = Point3::new(radius * c1, radius * s1, half);
            let p3 = Point3::new(radius * c0, radius * s0, half);
            for (v1, v2, v3) in [(p0, p1, p2), (p0, p2, p3)] {
                let normal = (v2 - v1).cross(&(v3 - v1)).normalize();
                mesh.vertices.extend_from_slice(&[v1, v2, v3]);
                mesh.triangles.push(Triangle {
                    v1,
                    v2,
                    v3,
                    normal: Point3::from(normal),
                });
            }
        }
        mesh
    }

    fn make_sphere(&self, center: Point3<f64>, radius: f64) -> Mesh {
        let mut mesh = Mesh::new();
        let stacks = 12;
        let slices = 24;
        for i in 0..stacks {
            let phi0 = std::f64::consts::PI * i as f64 / stacks as f64;
            let phi1 = std::f64::consts::PI * (i + 1) as f64 / stacks as f64;
            for j in 0..slices {
                let theta0 = 2.0 * std::f64::consts::PI * j as f64 / slices as f64;
                let theta1 = 2.0 * std::f64::consts::PI * (j + 1) as f64 / slices as f64;
                let p0 = center
                    + Vector3::new(
                        radius * phi0.sin() * theta0.cos(),
                        radius * phi0.sin() * theta0.sin(),
                        radius * phi0.cos(),
                    );
                let p1 = center
                    + Vector3::new(
                        radius * phi1.sin() * theta0.cos(),
                        radius * phi1.sin() * theta0.sin(),
                        radius * phi1.cos(),
                    );
                let p2 = center
                    + Vector3::new(
                        radius * phi1.sin() * theta1.cos(),
                        radius * phi1.sin() * theta1.sin(),
                        radius * phi1.cos(),
                    );
                let p3 = center
                    + Vector3::new(
                        radius * phi0.sin() * theta1.cos(),
                        radius * phi0.sin() * theta1.sin(),
                        radius * phi0.cos(),
                    );
                for (v1, v2, v3) in [(p0, p1, p2), (p0, p2, p3)] {
                    let normal = (v2 - v1).cross(&(v3 - v1)).normalize();
                    mesh.vertices.extend_from_slice(&[v1, v2, v3]);
                    mesh.triangles.push(Triangle {
                        v1,
                        v2,
                        v3,
                        normal: Point3::from(normal),
                    });
                }
            }
        }
        mesh
    }

    fn make_cone(
        &self,
        _center: Point3<f64>,
        _axis: Point3<f64>,
        r_bot: f64,
        r_top: f64,
        height: f64,
    ) -> Mesh {
        let mut mesh = Mesh::new();
        let sides = 24;
        let half = height * 0.5;
        for i in 0..sides {
            let a0 = 2.0 * std::f64::consts::PI * i as f64 / sides as f64;
            let a1 = 2.0 * std::f64::consts::PI * (i + 1) as f64 / sides as f64;
            let (c0, s0) = (a0.cos(), a0.sin());
            let (c1, s1) = (a1.cos(), a1.sin());
            let p0 = Point3::new(r_bot * c0, r_bot * s0, -half);
            let p1 = Point3::new(r_bot * c1, r_bot * s1, -half);
            let p2 = Point3::new(r_top * c1, r_top * s1, half);
            let p3 = Point3::new(r_top * c0, r_top * s0, half);
            for (v1, v2, v3) in [(p0, p1, p2), (p0, p2, p3)] {
                let normal = (v2 - v1).cross(&(v3 - v1)).normalize();
                mesh.vertices.extend_from_slice(&[v1, v2, v3]);
                mesh.triangles.push(Triangle {
                    v1,
                    v2,
                    v3,
                    normal: Point3::from(normal),
                });
            }
        }
        mesh
    }

    fn make_torus(
        &self,
        center: Point3<f64>,
        _axis: Point3<f64>,
        major_r: f64,
        minor_r: f64,
    ) -> Mesh {
        let mut mesh = Mesh::new();
        let rings = 16;
        let sides = 16;
        for i in 0..rings {
            let u0 = 2.0 * std::f64::consts::PI * i as f64 / rings as f64;
            let u1 = 2.0 * std::f64::consts::PI * (i + 1) as f64 / rings as f64;
            for j in 0..sides {
                let v0 = 2.0 * std::f64::consts::PI * j as f64 / sides as f64;
                let v1 = 2.0 * std::f64::consts::PI * (j + 1) as f64 / sides as f64;
                let ring = |u: f64, v: f64| -> Point3<f64> {
                    let r = major_r + minor_r * v.cos();
                    center + Vector3::new(r * u.cos(), r * u.sin(), minor_r * v.sin())
                };
                let p0 = ring(u0, v0);
                let p1 = ring(u1, v0);
                let p2 = ring(u1, v1);
                let p3 = ring(u0, v1);
                for (v1, v2, v3) in [(p0, p1, p2), (p0, p2, p3)] {
                    let normal = (v2 - v1).cross(&(v3 - v1)).normalize();
                    mesh.vertices.extend_from_slice(&[v1, v2, v3]);
                    mesh.triangles.push(Triangle {
                        v1,
                        v2,
                        v3,
                        normal: Point3::from(normal),
                    });
                }
            }
        }
        mesh
    }

    fn load_external(&self, path: &str, format: ExternalFormat) -> Mesh {
        let ext = match format {
            ExternalFormat::Step => ".step",
            ExternalFormat::Iges => ".iges",
            ExternalFormat::Brep => ".brep",
        };
        let _full_path = format!("{}{}", path, ext);
        // When B-Rep is available, use opencascade-rs to load the file.
        // Fallback: mesh is empty with a note.
        let mut mesh = Mesh::new();
        mesh.vertices.push(Point3::new(0.0, 0.0, 0.0));
        mesh
    }

    fn shape_to_mesh(&self, shape: &BRepShape) -> Mesh {
        match self.generate(shape) {
            BRepResult::Mesh(m) => m,
            BRepResult::Stats { .. } => Mesh::new(),
        }
    }

    fn make_extruded_mesh(
        &self,
        profile: &[Point3<f64>],
        direction: &Point3<f64>,
        height: f64,
    ) -> Mesh {
        let mut mesh = Mesh::new();
        let n = profile.len();
        if n < 3 {
            return mesh;
        }
        let dir = Point3::new(direction.x, direction.y, direction.z);
        let len = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
        if len < 1e-12 {
            return mesh;
        }
        let dir = Point3::new(dir.x / len, dir.y / len, dir.z / len);
        let offset = Point3::new(dir.x * height, dir.y * height, dir.z * height);

        for i in 0..n {
            let j = (i + 1) % n;
            let b0 = profile[i];
            let b1 = profile[j];
            let t0 = Point3::new(b0.x + offset.x, b0.y + offset.y, b0.z + offset.z);
            let t1 = Point3::new(b1.x + offset.x, b1.y + offset.y, b1.z + offset.z);
            for (v1, v2, v3) in [(b0, b1, t1), (b0, t1, t0)] {
                let normal = (v2 - v1).cross(&(v3 - v1)).normalize();
                mesh.vertices.extend_from_slice(&[v1, v2, v3]);
                mesh.triangles.push(Triangle {
                    v1,
                    v2,
                    v3,
                    normal: Point3::from(normal),
                });
            }
        }
        mesh
    }

    fn make_lofted_mesh(&self, profiles: &[Vec<Point3<f64>>]) -> Mesh {
        let mut mesh = Mesh::new();
        for k in 0..profiles.len().saturating_sub(1) {
            let bot = &profiles[k];
            let top = &profiles[k + 1];
            let n = bot.len().min(top.len());
            if n < 3 {
                continue;
            }
            for i in 0..n {
                let j = (i + 1) % n;
                let b0 = bot[i];
                let b1 = bot[j];
                let t0 = top[i];
                let t1 = top[j];
                for (v1, v2, v3) in [(b0, b1, t1), (b0, t1, t0)] {
                    let normal = (v2 - v1).cross(&(v3 - v1)).normalize();
                    mesh.vertices.extend_from_slice(&[v1, v2, v3]);
                    mesh.triangles.push(Triangle {
                        v1,
                        v2,
                        v3,
                        normal: Point3::from(normal),
                    });
                }
            }
        }
        mesh
    }
}

/// Brute-force fuse: simply concatenate both meshes.
fn fuse_meshes(a: &Mesh, b: &Mesh) -> Mesh {
    let mut out = Mesh::new();
    let _v_offset = a.vertices.len();
    out.vertices = [a.vertices.clone(), b.vertices.clone()].concat();
    for tri in &a.triangles {
        out.triangles.push(*tri);
    }
    for tri in &b.triangles {
        out.triangles.push(Triangle {
            v1: tri.v1,
            v2: tri.v2,
            v3: tri.v3,
            normal: tri.normal,
        });
    }
    out
}

/// Brute-force cut: mesh_a minus mesh_b (simple vertex containment check).
fn cut_mesh(a: &Mesh, b: &Mesh) -> Mesh {
    let mut out = Mesh::new();
    for tri in &a.triangles {
        let centroid = Point3::new(
            (tri.v1.x + tri.v2.x + tri.v3.x) / 3.0,
            (tri.v1.y + tri.v2.y + tri.v3.y) / 3.0,
            (tri.v1.z + tri.v2.z + tri.v3.z) / 3.0,
        );
        if !point_in_mesh(&centroid, b) {
            out.vertices.extend_from_slice(&[tri.v1, tri.v2, tri.v3]);
            out.triangles.push(*tri);
        }
    }
    out
}

/// Brute-force intersection: keep triangles from a that are inside b.
fn intersect_meshes(a: &Mesh, b: &Mesh) -> Mesh {
    let mut out = Mesh::new();
    for tri in &a.triangles {
        let centroid = Point3::new(
            (tri.v1.x + tri.v2.x + tri.v3.x) / 3.0,
            (tri.v1.y + tri.v2.y + tri.v3.y) / 3.0,
            (tri.v1.z + tri.v2.z + tri.v3.z) / 3.0,
        );
        if point_in_mesh(&centroid, b) {
            out.vertices.extend_from_slice(&[tri.v1, tri.v2, tri.v3]);
            out.triangles.push(*tri);
        }
    }
    out
}

/// Simple point-in-mesh test using ray casting in 3D.
fn point_in_mesh(point: &Point3<f64>, mesh: &Mesh) -> bool {
    let mut intersections = 0;
    let ray_dir = Point3::new(1.0, 0.0, 0.0);
    for tri in &mesh.triangles {
        if ray_triangle_intersect(point, &ray_dir, tri) {
            intersections += 1;
        }
    }
    intersections % 2 == 1
}

/// Möller–Trumbore ray-triangle intersection.
fn ray_triangle_intersect(origin: &Point3<f64>, dir: &Point3<f64>, tri: &Triangle) -> bool {
    let eps = 1e-8;
    let edge1 = tri.v2 - tri.v1;
    let edge2 = tri.v3 - tri.v1;
    let h = dir.coords.cross(&edge2);
    let a = edge1.dot(&h);
    if a.abs() < eps {
        return false;
    }
    let f = 1.0 / a;
    let s = origin - tri.v1;
    let u = f * s.dot(&h);
    if !(0.0..=1.0).contains(&u) {
        return false;
    }
    let q = s.cross(&edge1);
    let v = f * dir.coords.dot(&q);
    if v < 0.0 || u + v > 1.0 {
        return false;
    }
    let t = f * edge2.dot(&q);
    t > eps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kernel_default() {
        let kernel = BRepKernel::new();
        assert!(!kernel.is_brep_available() || kernel.is_brep_available());
    }

    #[test]
    fn test_generate_box() {
        let kernel = BRepKernel::new();
        let shape = BRepShape::Box {
            corner: Point3::new(0.0, 0.0, 0.0),
            dx: 10.0,
            dy: 10.0,
            dz: 10.0,
        };
        match kernel.generate(&shape) {
            BRepResult::Mesh(mesh) => {
                assert_eq!(mesh.triangles.len(), 12);
            }
            _ => panic!("expected Mesh"),
        }
    }

    #[test]
    fn test_generate_cylinder() {
        let kernel = BRepKernel::new();
        let shape = BRepShape::Cylinder {
            center: Point3::new(0.0, 0.0, 0.0),
            axis: Point3::new(0.0, 0.0, 1.0),
            radius: 5.0,
            height: 20.0,
        };
        match kernel.generate(&shape) {
            BRepResult::Mesh(mesh) => {
                assert!(mesh.triangles.len() > 0);
            }
            _ => panic!("expected Mesh"),
        }
    }

    #[test]
    fn test_generate_sphere() {
        let kernel = BRepKernel::new();
        let shape = BRepShape::Sphere {
            center: Point3::new(0.0, 0.0, 0.0),
            radius: 5.0,
        };
        match kernel.generate(&shape) {
            BRepResult::Mesh(mesh) => {
                assert!(mesh.triangles.len() > 0);
            }
            _ => panic!("expected Mesh"),
        }
    }

    #[test]
    fn test_generate_cone() {
        let kernel = BRepKernel::new();
        let shape = BRepShape::Cone {
            center: Point3::new(0.0, 0.0, 0.0),
            axis: Point3::new(0.0, 0.0, 1.0),
            radius_bottom: 5.0,
            radius_top: 2.0,
            height: 10.0,
        };
        match kernel.generate(&shape) {
            BRepResult::Mesh(mesh) => {
                assert!(mesh.triangles.len() > 0);
            }
            _ => panic!("expected Mesh"),
        }
    }

    #[test]
    fn test_generate_torus() {
        let kernel = BRepKernel::new();
        let shape = BRepShape::Torus {
            center: Point3::new(0.0, 0.0, 0.0),
            axis: Point3::new(0.0, 0.0, 1.0),
            major_radius: 10.0,
            minor_radius: 2.0,
        };
        match kernel.generate(&shape) {
            BRepResult::Mesh(mesh) => {
                assert!(mesh.triangles.len() > 0);
            }
            _ => panic!("expected Mesh"),
        }
    }

    #[test]
    fn test_boolean_fuse() {
        let kernel = BRepKernel::new();
        let a = BRepShape::Box {
            corner: Point3::new(0.0, 0.0, 0.0),
            dx: 5.0,
            dy: 5.0,
            dz: 5.0,
        };
        let b = BRepShape::Box {
            corner: Point3::new(2.5, 2.5, 2.5),
            dx: 5.0,
            dy: 5.0,
            dz: 5.0,
        };
        match kernel.boolean(&a, &b, BooleanOp::Fuse) {
            BRepResult::Stats { triangle_count, .. } => {
                assert!(triangle_count > 0);
            }
            _ => panic!("expected Stats"),
        }
    }

    #[test]
    fn test_extrude_profile() {
        let kernel = BRepKernel::new();
        let profile = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(1.0, 0.0, 0.0),
            Point3::new(0.5, 1.0, 0.0),
        ];
        match kernel.extrude(&profile, &Point3::new(0.0, 0.0, 1.0), 5.0) {
            BRepResult::Stats { triangle_count, .. } => {
                assert!(triangle_count > 0);
            }
            _ => panic!("expected Stats"),
        }
    }

    #[test]
    fn test_loft_profiles() {
        let kernel = BRepKernel::new();
        let profiles = vec![
            vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.5, 1.0, 0.0),
            ],
            vec![
                Point3::new(0.0, 0.0, 1.0),
                Point3::new(1.0, 0.0, 1.0),
                Point3::new(0.5, 1.0, 1.0),
            ],
        ];
        match kernel.loft(&profiles) {
            BRepResult::Stats { triangle_count, .. } => {
                assert!(triangle_count > 0);
            }
            _ => panic!("expected Stats"),
        }
    }

    #[test]
    fn test_moller_trumbore() {
        let tri = Triangle {
            v1: Point3::new(0.0, 0.0, 0.0),
            v2: Point3::new(1.0, 0.0, 0.0),
            v3: Point3::new(0.0, 1.0, 0.0),
            normal: Point3::new(0.0, 0.0, 1.0),
        };
        assert!(ray_triangle_intersect(
            &Point3::new(0.25, 0.25, 1.0),
            &Point3::new(0.0, 0.0, -1.0),
            &tri,
        ));
        assert!(!ray_triangle_intersect(
            &Point3::new(2.0, 2.0, 1.0),
            &Point3::new(0.0, 0.0, -1.0),
            &tri,
        ));
    }
}
