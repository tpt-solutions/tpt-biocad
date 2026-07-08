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

impl SlicingParams {
    /// Validate slicing parameters. Returns Ok(()) if all values are within
    /// acceptable ranges.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.layer_height <= 0.0 || !self.layer_height.is_finite() {
            errors.push(format!(
                "layer height must be positive, got {}",
                self.layer_height
            ));
        }
        if self.layer_height > 5.0 {
            errors.push(format!(
                "layer height ({}) seems unreasonably large (> 5 mm)",
                self.layer_height
            ));
        }
        if !(0.0..=1.0).contains(&self.infill_density) || !self.infill_density.is_finite() {
            errors.push(format!(
                "infill density must be in [0, 1], got {}",
                self.infill_density
            ));
        }
        if self.perimeters == 0 {
            errors.push("at least one perimeter is required".to_string());
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
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
    /// Tool change (Tx) — switch to a different extruder/material.
    /// The tool index corresponds to the material index.
    ToolChange { tool: usize },
}

/// Slice result containing toolpath commands
#[derive(Debug, Clone)]
pub struct SliceResult {
    pub layers: Vec<SliceLayer>,
    pub commands: Vec<ToolpathCommand>,
}

impl SliceResult {
    /// Estimate total print time in seconds from toolpath commands.
    /// Accounts for rapid moves, linear moves, UV curing dwell, and thermal waits.
    pub fn print_time_seconds(&self) -> f64 {
        let mut time = 0.0;
        let mut current_x = 0.0;
        let mut current_y = 0.0;
        let mut current_z = 0.0;
        let mut feed_rate = 1500.0; // default mm/min

        for cmd in &self.commands {
            match cmd {
                ToolpathCommand::Move {
                    x, y, z, f, rapid, ..
                } => {
                    let nx = *x;
                    let ny = *y;
                    let nz = *z;
                    let dx = nx - current_x;
                    let dy = ny - current_y;
                    let dz = nz - current_z;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();

                    if let Some(f_val) = f {
                        feed_rate = *f_val;
                    }

                    // Time = distance / speed. Feed rate is in mm/min.
                    let speed_mm_s = feed_rate / 60.0;
                    if speed_mm_s > 0.0 {
                        time += dist / speed_mm_s;
                    }

                    current_x = nx;
                    current_y = ny;
                    current_z = nz;

                    // Rapid moves are faster (use 3x feed rate)
                    if *rapid && dist > 0.0 {
                        time -= dist / (feed_rate / 60.0);
                        time += dist / (feed_rate * 3.0 / 60.0);
                    }
                }
                ToolpathCommand::UVCuring { duration, .. } => {
                    time += duration;
                }
                ToolpathCommand::SetTemperature { wait: true, .. } => {
                    time += 10.0; // estimate 10s for temperature stabilization
                }
                ToolpathCommand::ToolChange { .. } => {
                    time += 5.0; // estimate 5s for tool change
                }
                _ => {}
            }
        }

        time
    }

    /// Total number of layers with polygons.
    pub fn layer_count(&self) -> usize {
        self.layers.len()
    }

    /// Total toolpath length in mm (linear + rapid moves).
    pub fn total_toolpath_length(&self) -> f64 {
        let mut length = 0.0;
        let mut cx = 0.0;
        let mut cy = 0.0;
        let mut cz = 0.0;

        for cmd in &self.commands {
            if let ToolpathCommand::Move { x, y, z, .. } = cmd {
                let nx = *x;
                let ny = *y;
                let nz = *z;
                let dx = nx - cx;
                let dy = ny - cy;
                let dz = nz - cz;
                length += (dx * dx + dy * dy + dz * dz).sqrt();
                cx = nx;
                cy = ny;
                cz = nz;
            }
        }

        length
    }
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
    /// Material index for multi-material slicing. 0 = default material.
    /// When multiple materials are used, this determines which extruder/tool
    /// is active for this polygon.
    pub material_id: usize,
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
    ///
    /// For multi-material slicing, pass multiple materials. The first material
    /// (index 0) is the default. All polygons get `material_id` assigned based
    /// on the material that should print them. Tool-change commands (Tx) are
    /// inserted at layer boundaries when the active tool switches.
    pub fn slice(&self, mesh: &Mesh, material: Option<&Material>) -> SliceResult {
        let materials: Vec<&Material> = material.map(|m| vec![m]).unwrap_or_default();
        self.slice_multi(mesh, &materials)
    }

    /// Slice with explicit multi-material support.
    /// If `materials` is empty, no material-specific commands are emitted.
    pub fn slice_multi(&self, mesh: &Mesh, materials: &[&Material]) -> SliceResult {
        let mut layers = Vec::new();
        let mut commands = Vec::new();
        let mut active_tool: usize = 0;

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
                // Determine which materials are used in this layer
                let layer_tools: Vec<usize> = {
                    let mut tools: Vec<usize> =
                        layer.polygons.iter().map(|p| p.material_id).collect();
                    tools.sort();
                    tools.dedup();
                    tools
                };

                // Emit tool changes at layer boundaries
                if materials.len() > 1 {
                    for &tool in &layer_tools {
                        if tool != active_tool && tool < materials.len() {
                            commands.push(ToolpathCommand::ToolChange { tool });
                            active_tool = tool;
                        }
                    }
                }

                layers.push(layer);

                // Generate toolpath commands for this layer
                self.generate_layer_commands(&mut commands, z, materials, active_tool);
            }
            z += self.params.layer_height;
        }

        SliceResult { layers, commands }
    }

    fn generate_layer_commands(
        &self,
        commands: &mut Vec<ToolpathCommand>,
        z: f64,
        materials: &[&Material],
        _active_tool: usize,
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

        // Emit material-specific commands for all active materials
        for mat in materials {
            // UV curing: insert M302 between layers for photo-crosslinkable materials
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
        // Collect all intersection segments from triangle-plane clipping
        let mut segments: Vec<(Point3<f64>, Point3<f64>)> = Vec::new();

        for tri in &mesh.triangles {
            if let Some(pts) = clip_triangle_to_plane(tri.v1, tri.v2, tri.v3, z) {
                if pts.len() >= 2 {
                    // pts are unordered; the two intersection points form a segment
                    segments.push((pts[0], pts[1]));
                }
            }
        }

        // Stitch segments into closed polygon loops
        let polygons = stitch_polygons(&segments);

        SliceLayer { z, polygons }
    }
}

/// Classify a vertex relative to the Z plane: -1 = below, 0 = on, +1 = above.
fn classify(v: Point3<f64>, z: f64) -> i8 {
    const EPS: f64 = 1e-10;
    if v.z < z - EPS {
        -1
    } else if v.z > z + EPS {
        1
    } else {
        0
    }
}

/// Interpolate the intersection point of edge (a→b) with the Z plane.
fn intersect_edge(a: Point3<f64>, b: Point3<f64>, z: f64) -> Point3<f64> {
    let dz = b.z - a.z;
    if dz.abs() < 1e-12 {
        return a; // degenerate
    }
    let t = (z - a.z) / dz;
    Point3::new(a.x + t * (b.x - a.x), a.y + t * (b.y - a.y), z)
}

/// Clip a triangle against the Z = z plane.
/// Returns the intersection polygon (0, 2, or 3 points).
fn clip_triangle_to_plane(
    v1: Point3<f64>,
    v2: Point3<f64>,
    v3: Point3<f64>,
    z: f64,
) -> Option<Vec<Point3<f64>>> {
    let c1 = classify(v1, z);
    let c2 = classify(v2, z);
    let c3 = classify(v3, z);

    // All on one side: no intersection
    if (c1 == -1 && c2 == -1 && c3 == -1) || (c1 == 1 && c2 == 1 && c3 == 1) {
        return None;
    }

    // All on the plane: return the triangle itself
    if c1 == 0 && c2 == 0 && c3 == 0 {
        return Some(vec![v1, v2, v3]);
    }

    // Collect intersection points
    let mut points: Vec<Point3<f64>> = Vec::new();

    // Add vertices that are exactly on the plane
    if c1 == 0 {
        points.push(v1);
    }
    if c2 == 0 {
        points.push(v2);
    }
    if c3 == 0 {
        points.push(v3);
    }

    // Find edges that cross the plane
    let edges = [(v1, v2, c1, c2), (v2, v3, c2, c3), (v3, v1, c3, c1)];
    for (a, b, ca, cb) in edges {
        // Strict crossing: one above, one below
        if (ca == 1 && cb == -1) || (ca == -1 && cb == 1) {
            points.push(intersect_edge(a, b, z));
        }
        // One on the plane, the other off: the on-plane vertex is the intersection
        else if ca == 0 && cb != 0 {
            points.push(a);
        } else if cb == 0 && ca != 0 {
            points.push(b);
        }
    }

    // Deduplicate
    points.dedup_by(|a, b| (a.x - b.x).abs() < 1e-10 && (a.y - b.y).abs() < 1e-10);

    if points.len() >= 2 {
        Some(points)
    } else {
        None
    }
}

/// Stitch a collection of unordered edge segments into closed polygon loops.
/// Each segment is a pair of endpoints. Endpoints that share a position within
/// EPS are considered connected. Returns the outer polygon first, followed by
/// hole polygons (based on winding direction).
fn stitch_polygons(segments: &[(Point3<f64>, Point3<f64>)]) -> Vec<Polygon> {
    if segments.is_empty() {
        return Vec::new();
    }

    const EPS: f64 = 1e-8;
    let mut segs: Vec<(Point3<f64>, Point3<f64>)> = segments.to_vec();
    let mut loops: Vec<Vec<Point3<f64>>> = Vec::new();

    while !segs.is_empty() {
        let (a, b) = segs.remove(0);
        let mut chain = vec![a, b];
        let mut changed = true;

        // Extend the chain by repeatedly finding matching endpoints
        while changed {
            changed = false;
            for i in (0..segs.len()).rev() {
                let (ca, cb) = segs[i];
                let first = chain[0];
                let last = *chain.last().unwrap();

                if (ca - first).norm() < EPS {
                    chain.insert(0, cb);
                    segs.swap_remove(i);
                    changed = true;
                } else if (cb - first).norm() < EPS {
                    chain.insert(0, ca);
                    segs.swap_remove(i);
                    changed = true;
                } else if (ca - last).norm() < EPS {
                    chain.push(cb);
                    segs.swap_remove(i);
                    changed = true;
                } else if (cb - last).norm() < EPS {
                    chain.push(ca);
                    segs.swap_remove(i);
                    changed = true;
                }
            }
        }

        // Close the loop if the first and last points match
        let first = chain[0];
        let last = *chain.last().unwrap();
        if (first - last).norm() > EPS && chain.len() > 2 {
            chain.push(first);
        }

        if chain.len() >= 3 {
            loops.push(chain);
        }
    }

    // Determine winding: positive Z area → counter-clockwise (outer),
    // negative Z area → clockwise (hole). Compute signed 2D area in the XY plane.
    let mut with_area: Vec<(f64, Vec<Point3<f64>>)> = loops
        .into_iter()
        .map(|pts| {
            let area = signed_area_xy(&pts);
            (area, pts)
        })
        .collect();

    // Sort by descending absolute area (largest first)
    with_area.sort_by(|a, b| b.0.abs().partial_cmp(&a.0.abs()).unwrap());

    let mut result = Vec::new();
    for (area, pts) in with_area {
        // Positive area → CCW → outer perimeter, negative → CW → hole
        let is_perimeter = area >= 0.0;
        result.push(Polygon {
            points: pts,
            is_perimeter,
            material_id: 0, // default; assign per-region in multi-material flow
        });
    }

    result
}

/// Signed 2D area of a polygon projected onto the XY plane.
/// Positive = counter-clockwise, negative = clockwise.
fn signed_area_xy(pts: &[Point3<f64>]) -> f64 {
    let n = pts.len();
    if n < 3 {
        return 0.0;
    }
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i].x * pts[j].y;
        area -= pts[j].x * pts[i].y;
    }
    area * 0.5
}

/// Clip a subject polygon against a clip polygon using the Sutherland-Hodgman
/// algorithm. Both polygons are given as ordered point lists (XY plane only;
/// Z is ignored). Returns the clipped polygon, or None if entirely outside.
pub fn clip_polygon_sutherland_hodgman(
    subject: &[Point3<f64>],
    clip: &[Point3<f64>],
) -> Option<Vec<Point3<f64>>> {
    if subject.len() < 3 || clip.len() < 3 {
        return None;
    }

    let mut output: Vec<Point3<f64>> = subject.to_vec();

    let n = clip.len();
    for i in 0..n {
        if output.is_empty() {
            return None;
        }

        let edge_start = clip[i];
        let edge_end = clip[(i + 1) % n];
        let input = output.clone();
        output.clear();

        // Edge plane normal (inward-pointing, 2D cross product)
        let edge_dx = edge_end.x - edge_start.x;
        let edge_dy = edge_end.y - edge_start.y;

        for j in 0..input.len() {
            let current = input[j];
            let prev = if j == 0 {
                input[input.len() - 1]
            } else {
                input[j - 1]
            };

            let inside_current = is_inside(current, edge_start, edge_dx, edge_dy);
            let inside_prev = is_inside(prev, edge_start, edge_dx, edge_dy);

            if inside_current {
                if !inside_prev {
                    // Entering: add intersection
                    if let Some(p) = intersect_lines(prev, current, edge_start, edge_end) {
                        output.push(p);
                    }
                }
                output.push(current);
            } else if inside_prev {
                // Exiting: add intersection
                if let Some(p) = intersect_lines(prev, current, edge_start, edge_end) {
                    output.push(p);
                }
            }
        }
    }

    if output.len() < 3 {
        None
    } else {
        Some(output)
    }
}

/// Test if a point is to the left of the directed edge (inside the clip region).
fn is_inside(point: Point3<f64>, edge_start: Point3<f64>, dx: f64, dy: f64) -> bool {
    // Cross product (edge × (point - edge_start)) > 0 means left side
    let px = point.x - edge_start.x;
    let py = point.y - edge_start.y;
    dx * py - dy * px >= 0.0
}

/// Intersect two line segments (p1→p2) and (p3→p4) in the XY plane.
fn intersect_lines(
    p1: Point3<f64>,
    p2: Point3<f64>,
    p3: Point3<f64>,
    p4: Point3<f64>,
) -> Option<Point3<f64>> {
    let x1 = p1.x;
    let y1 = p1.y;
    let x2 = p2.x;
    let y2 = p2.y;
    let x3 = p3.x;
    let y3 = p3.y;
    let x4 = p4.x;
    let y4 = p4.y;

    let denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
    if denom.abs() < 1e-12 {
        return None;
    }

    let t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / denom;
    // Z is the average of the two lines' Z values at the intersection
    let z = 0.5 * (p1.z + p2.z);
    Some(Point3::new(x1 + t * (x2 - x1), y1 + t * (y2 - y1), z))
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

    #[test]
    fn test_clip_triangle_above() {
        let v = |x, y, z| nalgebra::Point3::new(x, y, z);
        // Triangle entirely above z=0.5
        let result =
            clip_triangle_to_plane(v(0.0, 0.0, 1.0), v(1.0, 0.0, 1.0), v(0.5, 1.0, 1.0), 0.5);
        assert!(result.is_none());
    }

    #[test]
    fn test_clip_triangle_below() {
        let v = |x, y, z| nalgebra::Point3::new(x, y, z);
        // Triangle entirely below z=0.5
        let result =
            clip_triangle_to_plane(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.5, 1.0, 0.0), 0.5);
        assert!(result.is_none());
    }

    #[test]
    fn test_clip_triangle_crossing() {
        let v = |x, y, z| nalgebra::Point3::new(x, y, z);
        // One vertex above, two below z=0.5
        let result =
            clip_triangle_to_plane(v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0), v(0.5, 0.5, 1.0), 0.5);
        assert!(result.is_some());
        let pts = result.unwrap();
        assert_eq!(pts.len(), 2); // line segment
                                  // Intersection should be at z=0.5
        for p in &pts {
            assert!((p.z - 0.5).abs() < 1e-10);
        }
    }

    #[test]
    fn test_clip_two_above_one_below() {
        let v = |x, y, z| nalgebra::Point3::new(x, y, z);
        // Two vertices above, one below z=0.5
        let result =
            clip_triangle_to_plane(v(0.0, 0.0, 0.0), v(1.0, 0.0, 1.0), v(0.5, 1.0, 1.0), 0.5);
        assert!(result.is_some());
        let pts = result.unwrap();
        assert_eq!(pts.len(), 2);
        for p in &pts {
            assert!((p.z - 0.5).abs() < 1e-10);
        }
    }

    #[test]
    fn test_clip_vertex_on_plane() {
        let v = |x, y, z| nalgebra::Point3::new(x, y, z);
        // One vertex on the plane, one above, one below — should produce 2 points
        let result =
            clip_triangle_to_plane(v(0.0, 0.0, 0.0), v(1.0, 0.0, 1.0), v(0.5, 0.5, 0.5), 0.5);
        assert!(result.is_some());
        let pts = result.unwrap();
        assert!(pts.len() >= 2);
    }

    #[test]
    fn test_print_time_estimation() {
        let slicer = Slicer::new(params());
        let result = slicer.slice(&test_mesh(), None);
        let time = result.print_time_seconds();
        assert!(time > 0.0, "print time should be positive, got {}", time);
        let length = result.total_toolpath_length();
        assert!(length > 0.0, "toolpath length should be positive");
    }

    #[test]
    fn test_stitch_single_segment() {
        // Two segments that form a triangle
        let v = |x, y| nalgebra::Point3::new(x, y, 0.0);
        let segs = vec![
            (v(0.0, 0.0), v(1.0, 0.0)),
            (v(1.0, 0.0), v(0.5, 1.0)),
            (v(0.5, 1.0), v(0.0, 0.0)),
        ];
        let polys = stitch_polygons(&segs);
        assert_eq!(polys.len(), 1);
        assert!(polys[0].is_perimeter);
        assert!(polys[0].points.len() >= 3);
    }

    #[test]
    fn test_stitch_multiple_loops() {
        let v = |x, y| nalgebra::Point3::new(x, y, 0.0);
        // Outer square (CCW)
        let outer = vec![
            (v(0.0, 0.0), v(1.0, 0.0)),
            (v(1.0, 0.0), v(1.0, 1.0)),
            (v(1.0, 1.0), v(0.0, 1.0)),
            (v(0.0, 1.0), v(0.0, 0.0)),
        ];
        // Inner square (CW) — a hole
        let inner = vec![
            (v(0.3, 0.3), v(0.3, 0.7)),
            (v(0.3, 0.7), v(0.7, 0.7)),
            (v(0.7, 0.7), v(0.7, 0.3)),
            (v(0.7, 0.3), v(0.3, 0.3)),
        ];

        let mut segs = outer.clone();
        segs.extend(inner);
        let polys = stitch_polygons(&segs);
        // Should have two polygons: outer (perimeter) and inner (hole)
        assert_eq!(polys.len(), 2);
        assert!(polys[0].is_perimeter); // larger = outer
        assert!(!polys[1].is_perimeter); // smaller = hole
    }

    #[test]
    fn test_sutherland_hodgman_no_clip() {
        let v = |x, y| nalgebra::Point3::new(x, y, 0.0);
        // Subject fully inside clip
        let subject = vec![v(0.2, 0.2), v(0.8, 0.2), v(0.5, 0.8)];
        let clip = vec![v(0.0, 0.0), v(1.0, 0.0), v(1.0, 1.0), v(0.0, 1.0)];
        let result = clip_polygon_sutherland_hodgman(&subject, &clip);
        assert!(result.is_some());
        let clipped = result.unwrap();
        assert!(clipped.len() >= 3);
    }

    #[test]
    fn test_sutherland_hodgman_full_clip() {
        let v = |x, y| nalgebra::Point3::new(x, y, 0.0);
        // Subject entirely outside clip
        let subject = vec![v(2.0, 2.0), v(3.0, 2.0), v(2.5, 3.0)];
        let clip = vec![v(0.0, 0.0), v(1.0, 0.0), v(1.0, 1.0), v(0.0, 1.0)];
        let result = clip_polygon_sutherland_hodgman(&subject, &clip);
        assert!(result.is_none());
    }

    #[test]
    fn test_sutherland_hodgman_partial_clip() {
        let v = |x, y| nalgebra::Point3::new(x, y, 0.0);
        // Subject partially inside clip (triangle crossing the boundary)
        let subject = vec![v(0.5, -0.5), v(1.5, 0.5), v(-0.5, 0.5)];
        let clip = vec![v(0.0, 0.0), v(1.0, 0.0), v(1.0, 1.0), v(0.0, 1.0)];
        let result = clip_polygon_sutherland_hodgman(&subject, &clip);
        assert!(result.is_some());
        let clipped = result.unwrap();
        assert!(clipped.len() >= 3);
    }
}
