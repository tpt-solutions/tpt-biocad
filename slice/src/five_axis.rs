// 5-axis toolpath generation for curved mandrels (vascular bioprinting)
// Licensed under Apache 2.0
//
// Generates conformal toolpaths that follow the surface of a curved mandrel,
// maintaining constant nozzle offset and material deposition angle. This is
// essential for vascular bioprinting where the nozzle must stay normal to
// the rotating mandrel surface.

use nalgebra::{Point3, Vector3};
use tpt_core::InfillPattern;

/// Geometric representation of a mandrel for 5-axis printing.
#[derive(Debug, Clone)]
pub struct Mandrel {
    /// Centerline of the mandrel (ordered points defining the sweep path).
    pub centerline: Vec<Point3<f64>>,
    /// Radius at each centerline point (mm).
    pub radius: Vec<f64>,
    /// Total length along the centerline (mm).
    pub total_length: f64,
}

impl Mandrel {
    /// Create a straight cylindrical mandrel.
    pub fn cylinder(length: f64, radius: f64, segments: usize) -> Self {
        let t = segments.max(2);
        let centerline: Vec<Point3<f64>> = (0..=t)
            .map(|i| Point3::new(0.0, 0.0, length * i as f64 / t as f64))
            .collect();
        let radii = vec![radius; centerline.len()];
        Self {
            total_length: length,
            centerline,
            radius: radii,
        }
    }

    /// Create a curved mandrel from a set of control points (spline will
    /// be approximated by linear segments between points).
    pub fn from_points(points: Vec<Point3<f64>>, radius: f64) -> Self {
        let n = points.len().max(2);
        let radii = vec![radius; n];
        let mut total = 0.0;
        for i in 1..n {
            total += (points[i] - points[i - 1]).norm();
        }
        Self {
            total_length: total,
            centerline: points,
            radius: radii,
        }
    }

    /// Evaluate the mandrel surface normal and position at a given parametric
    /// distance along the centerline and a circumferential angle.
    pub fn surface_point(
        &self,
        arc_length: f64,
        theta: f64,
    ) -> Option<(Point3<f64>, Vector3<f64>)> {
        let (p, tangent) = self.interpolate(arc_length)?;
        // Build orthonormal basis perpendicular to tangent
        let up = if tangent.y.abs() < 0.9 {
            Vector3::new(0.0, 1.0, 0.0)
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };
        let e1 = up.cross(&tangent).normalize();
        let e2 = tangent.cross(&e1).normalize();

        let r = self.radius_at(arc_length)?;
        let normal = e1 * theta.cos() + e2 * theta.sin();
        let surface_pos = p + normal * r;

        Some((surface_pos, normal))
    }

    fn interpolate(&self, arc_length: f64) -> Option<(Point3<f64>, Vector3<f64>)> {
        if self.centerline.is_empty() {
            return None;
        }
        let s = arc_length.clamp(0.0, self.total_length);
        let mut accumulated = 0.0;
        for i in 1..self.centerline.len() {
            let seg = (self.centerline[i] - self.centerline[i - 1]).norm();
            if accumulated + seg >= s || i == self.centerline.len() - 1 {
                let t = if seg > 0.0 {
                    (s - accumulated) / seg
                } else {
                    0.0
                };
                let p = self.centerline[i - 1]
                    + (self.centerline[i] - self.centerline[i - 1]) * t.clamp(0.0, 1.0);
                let tangent = if seg > 0.0 {
                    (self.centerline[i] - self.centerline[i - 1]).normalize()
                } else {
                    Vector3::new(0.0, 0.0, 1.0)
                };
                return Some((p, tangent));
            }
            accumulated += seg;
        }
        None
    }

    fn radius_at(&self, arc_length: f64) -> Option<f64> {
        if self.radius.is_empty() {
            return None;
        }
        if self.radius.len() == 1 {
            return Some(self.radius[0]);
        }
        let s = arc_length.clamp(0.0, self.total_length);
        let seg_len = self.total_length / (self.radius.len() - 1) as f64;
        let idx = ((s / seg_len).floor() as usize).min(self.radius.len() - 2);
        let frac = (s - idx as f64 * seg_len) / seg_len;
        Some(self.radius[idx] * (1.0 - frac) + self.radius[idx + 1] * frac)
    }
}

/// Parameters for 5-axis slicing on a mandrel.
#[derive(Debug, Clone)]
pub struct FiveAxisParams {
    /// Layer height measured normal to the mandrel surface (mm).
    pub layer_height: f64,
    /// Nozzle offset from mandrel surface (mm). Positive = outside.
    pub nozzle_offset: f64,
    /// Print speed along toolpath (mm/s).
    pub print_speed: f64,
    /// Number of angular steps around the mandrel circumference.
    pub angular_segments: usize,
    /// Number of helical passes along the mandrel length.
    pub helical_passes: usize,
    /// Infill pattern for scaffold layers.
    pub infill_pattern: InfillPattern,
}

impl Default for FiveAxisParams {
    fn default() -> Self {
        Self {
            layer_height: 0.2,
            nozzle_offset: 0.1,
            print_speed: 5.0,
            angular_segments: 64,
            helical_passes: 1,
            infill_pattern: InfillPattern::Grid,
        }
    }
}

/// A single 5-axis move: position + orientation.
#[derive(Debug, Clone)]
pub struct FiveAxisMove {
    /// Tool tip position (mm).
    pub position: Point3<f64>,
    /// Tool orientation vector (should point from tool to surface).
    pub orientation: Vector3<f64>,
    /// Whether this is a rapid (travel) move.
    pub rapid: bool,
    /// Extrusion amount (mm).
    pub extrusion: Option<f64>,
    /// Feed rate (mm/min).
    pub feed_rate: Option<f64>,
}

/// Result of 5-axis slicing.
#[derive(Debug, Clone)]
pub struct FiveAxisSliceResult {
    /// Helical toolpath moves.
    pub moves: Vec<FiveAxisMove>,
    /// Estimated print time (seconds).
    pub estimated_time_s: f64,
    /// Total toolpath length (mm).
    pub total_length: f64,
    /// Number of revolutions around the mandrel.
    pub revolutions: usize,
}

/// Generate a 5-axis helical toolpath conformal to a mandrel surface.
///
/// This produces a continuous helical path that wraps around the mandrel
/// while keeping the nozzle normal to the surface. For vascular bioprinting
/// this enables printing bifurcating vessels on a rotating mandrel.
pub fn generate_five_axis_toolpath(
    mandrel: &Mandrel,
    params: &FiveAxisParams,
    start_angle: f64,
) -> FiveAxisSliceResult {
    let mut moves = Vec::new();
    let mut total_length = 0.0;

    let angular_resolution = params.angular_segments.max(8);
    let helical_passes = params.helical_passes.max(1);
    let total_length_mm = mandrel.total_length;
    let effective_radius = mandrel.radius.iter().cloned().fold(0.0, f64::max);

    // For each helical pass, generate conformal toolpath
    for pass in 0..helical_passes {
        let _z_offset = pass as f64 * params.layer_height;
        let _pitch = total_length_mm / angular_resolution as f64;
        let angular_step =
            2.0 * std::f64::consts::PI * helical_passes as f64 / angular_resolution as f64;

        let mut prev_pos: Option<Point3<f64>> = None;

        for i in 0..=angular_resolution {
            let arc_frac = i as f64 / angular_resolution as f64;
            let arc_length = arc_frac * total_length_mm;
            let theta = start_angle + i as f64 * angular_step;

            if let Some((surface_pt, normal)) = mandrel.surface_point(arc_length, theta) {
                let offset = normal * params.nozzle_offset;
                let tool_pos = surface_pt + offset;

                // Extrusion: approximate volume = cross_section * distance
                let extrusion = if let Some(prev) = prev_pos {
                    let dist = (tool_pos - prev).norm();
                    total_length += dist;
                    let cross_section = std::f64::consts::PI * effective_radius * effective_radius;
                    Some(dist * cross_section * 0.01) // scale factor
                } else {
                    None
                };

                moves.push(FiveAxisMove {
                    position: tool_pos,
                    orientation: normal,
                    rapid: i == 0 && pass == 0,
                    extrusion,
                    feed_rate: Some(params.print_speed * 60.0),
                });

                prev_pos = Some(tool_pos);
            }
        }
    }

    // Estimate time: sum distance / speed
    let estimated_time_s = if params.print_speed > 0.0 {
        total_length / params.print_speed
    } else {
        0.0
    };

    let revolutions = helical_passes;

    FiveAxisSliceResult {
        moves,
        estimated_time_s,
        total_length,
        revolutions,
    }
}

/// Convert a 5-axis toolpath to G-code with A/B axis rotations.
///
/// Uses G-code convention:
///   G1 X.. Y.. Z.. A.. B.. E.. F..
/// where A = rotation around X (pitch), B = rotation around Y (yaw).
/// The orientation vector is converted to Euler angles (in degrees).
pub fn five_axis_to_gcode(result: &FiveAxisSliceResult) -> String {
    let mut lines = vec![
        "; TPT BioCAD 5-axis G-code".to_string(),
        "; Conformal mandrel toolpath".to_string(),
        "; Licensed under Apache 2.0".to_string(),
    ];

    for mv in &result.moves {
        let (a_deg, b_deg) = orientation_to_euler(&mv.orientation);

        let mut parts = if mv.rapid {
            vec!["G0".to_string()]
        } else {
            vec!["G1".to_string()]
        };

        parts.push(format!("X{:.4}", mv.position.x));
        parts.push(format!("Y{:.4}", mv.position.y));
        parts.push(format!("Z{:.4}", mv.position.z));
        parts.push(format!("A{:.2}", a_deg));
        parts.push(format!("B{:.2}", b_deg));

        if let Some(e) = mv.extrusion {
            parts.push(format!("E{:.6}", e));
        }
        if let Some(f) = mv.feed_rate {
            parts.push(format!("F{:.0}", f));
        }

        lines.push(parts.join(" "));
    }

    lines.join("\n")
}

/// Convert an orientation vector to A/B Euler angles (degrees).
/// A = rotation around X axis (pitch), B = rotation around Y (yaw).
fn orientation_to_euler(orientation: &Vector3<f64>) -> (f64, f64) {
    let v = orientation.normalize();
    let a = v.y.atan2(v.z).to_degrees();
    let b = (-v.x).atan2((v.y * v.y + v.z * v.z).sqrt()).to_degrees();
    (a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mandrel_cylinder() {
        let mandrel = Mandrel::cylinder(10.0, 5.0, 20);
        assert_eq!(mandrel.centerline.len(), 21);
        assert!((mandrel.total_length - 10.0).abs() < 1e-6);
    }

    #[test]
    fn test_surface_point_cylinder() {
        let mandrel = Mandrel::cylinder(10.0, 5.0, 20);
        let (pt, normal) = mandrel.surface_point(5.0, 0.0).unwrap();
        // At theta=0, normal should be along +X
        assert!(normal.x > 0.0);
        assert!((normal.y - 0.0).abs() < 1e-6);
        // Surface point should be at (5, 0, 5)
        assert!((pt.x - 5.0).abs() < 1e-3);
        assert!((pt.y - 0.0).abs() < 1e-3);
        assert!((pt.z - 5.0).abs() < 1e-3);
    }

    #[test]
    fn test_five_axis_toolpath_generation() {
        let mandrel = Mandrel::cylinder(10.0, 5.0, 20);
        let params = FiveAxisParams::default();
        let result = generate_five_axis_toolpath(&mandrel, &params, 0.0);
        assert!(!result.moves.is_empty());
        assert!(result.total_length > 0.0);
        assert!(result.estimated_time_s > 0.0);
    }

    #[test]
    fn test_gcode_output() {
        let mandrel = Mandrel::cylinder(5.0, 3.0, 10);
        let params = FiveAxisParams {
            angular_segments: 8,
            ..Default::default()
        };
        let result = generate_five_axis_toolpath(&mandrel, &params, 0.0);
        let gcode = five_axis_to_gcode(&result);
        assert!(gcode.contains("G0") || gcode.contains("G1"));
        assert!(gcode.contains("A") && gcode.contains("B"));
    }

    #[test]
    fn test_curved_mandrel_from_points() {
        let points = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(0.5, 0.5, 2.0),
            Point3::new(0.0, 1.0, 4.0),
            Point3::new(-0.5, 1.5, 6.0),
        ];
        let mandrel = Mandrel::from_points(points, 3.0);
        assert!(mandrel.total_length > 0.0);
        let result = generate_five_axis_toolpath(&mandrel, &FiveAxisParams::default(), 0.0);
        assert!(!result.moves.is_empty());
    }

    #[test]
    fn test_orientation_to_euler_identity() {
        let v = Vector3::new(0.0, 0.0, 1.0);
        let (a, b) = orientation_to_euler(&v);
        assert!((a.abs() < 1e-6) && (b.abs() < 1e-6));
    }

    #[test]
    fn test_mandrel_interpolate_bounds() {
        let mandrel = Mandrel::cylinder(10.0, 5.0, 20);
        // At arc_length=0, should be at first point
        let (p0, _) = mandrel.interpolate(0.0).unwrap();
        assert!((p0 - mandrel.centerline[0]).norm() < 1e-6);
        // At arc_length=total, should be at last point
        let (p1, _) = mandrel.interpolate(mandrel.total_length).unwrap();
        assert!((p1 - mandrel.centerline[mandrel.centerline.len() - 1]).norm() < 1e-6);
    }
}
