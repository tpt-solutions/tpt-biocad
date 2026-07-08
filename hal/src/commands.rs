// G-code command builders for bioprinting/food printing
// Licensed under Apache 2.0
//
// Builds G-code strings for M300-M303 extended commands and standard
// extrusion commands, with extruder-type-specific behavior.

use tpt_core::ExtruderType;

/// Build an M300 pneumatic pressure command.
/// `pressure_kpa` — target pressure in kPa.
/// `duration_ms` — duration in milliseconds (0 = hold until next command).
pub fn pneumatic_pressure(pressure_kpa: f64, duration_ms: f64) -> String {
    format!("M300 S{:.0} P{:.0}", pressure_kpa, duration_ms)
}

/// Build an M301 thermal profiling command.
/// `temp_c` — target temperature in °C.
/// `ramp_rate` — ramp rate in °C/s.
pub fn thermal_profile(temp_c: f64, ramp_rate: f64) -> String {
    format!("M301 T{:.1} R{:.2}", temp_c, ramp_rate)
}

/// Build an M302 UV curing command.
/// `intensity_w_m2` — UV intensity in W/m².
/// `duration_s` — exposure time in seconds.
pub fn uv_cure(intensity_w_m2: f64, duration_s: f64) -> String {
    format!("M302 U{:.0} T{:.0}", intensity_w_m2, duration_s)
}

/// Build an M303 coaxial cross-linker command.
pub fn coaxial_flow() -> String {
    "M303".to_string()
}

/// Build a G1 extrusion command for screw/piston extruders.
/// `distance_mm` — extrusion distance in mm (negative for retract).
/// `speed_mm_s` — feed rate in mm/s.
pub fn extrude(distance_mm: f64, speed_mm_s: f64) -> String {
    format!("G1 E{:.4} F{:.0}", distance_mm, speed_mm_s * 60.0)
}

/// Build a retraction command.
pub fn retract(distance_mm: f64, speed_mm_s: f64) -> String {
    extrude(-distance_mm.abs(), speed_mm_s)
}

/// Build a G0 rapid move command.
pub fn rapid_move(x: Option<f64>, y: Option<f64>, z: Option<f64>) -> String {
    let mut parts = vec!["G0".to_string()];
    if let Some(v) = x {
        parts.push(format!("X{:.4}", v));
    }
    if let Some(v) = y {
        parts.push(format!("Y{:.4}", v));
    }
    if let Some(v) = z {
        parts.push(format!("Z{:.4}", v));
    }
    parts.join(" ")
}

/// Build a G1 linear move command.
pub fn linear_move(
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    e: Option<f64>,
    f: Option<f64>,
) -> String {
    let mut parts = vec!["G1".to_string()];
    if let Some(v) = x {
        parts.push(format!("X{:.4}", v));
    }
    if let Some(v) = y {
        parts.push(format!("Y{:.4}", v));
    }
    if let Some(v) = z {
        parts.push(format!("Z{:.4}", v));
    }
    if let Some(v) = e {
        parts.push(format!("E{:.4}", v));
    }
    if let Some(v) = f {
        parts.push(format!("F{:.0}", v));
    }
    parts.join(" ")
}

/// Build extrusion commands appropriate for the given extruder type.
///
/// For pneumatic extruders, uses M300 pressure control with a timed release.
/// For screw/piston extruders, uses G1 E-axis commands.
pub fn extruder_command(
    extruder: &ExtruderType,
    pressure_kpa: f64,
    distance_mm: f64,
    speed_mm_s: f64,
) -> Vec<String> {
    match extruder {
        ExtruderType::Pneumatic => {
            // Estimate extrusion time from distance and speed
            let extrusion_time_ms = if speed_mm_s > 0.0 {
                (distance_mm / speed_mm_s * 1000.0).ceil()
            } else {
                500.0 // default 500ms if speed is zero
            };
            vec![
                pneumatic_pressure(pressure_kpa, extrusion_time_ms + 100.0),
                extrude(distance_mm, speed_mm_s),
            ]
        }
        ExtruderType::Piston | ExtruderType::Screw => {
            vec![extrude(distance_mm, speed_mm_s)]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pneumatic_pressure() {
        assert_eq!(pneumatic_pressure(100.0, 500.0), "M300 S100 P500");
    }

    #[test]
    fn test_thermal_profile() {
        assert_eq!(thermal_profile(31.0, 0.5), "M301 T31.0 R0.50");
    }

    #[test]
    fn test_uv_cure() {
        assert_eq!(uv_cure(10.0, 30.0), "M302 U10 T30");
    }

    #[test]
    fn test_coaxial_flow() {
        assert_eq!(coaxial_flow(), "M303");
    }

    #[test]
    fn test_extrude() {
        assert_eq!(extrude(1.5, 10.0), "G1 E1.5000 F600");
    }

    #[test]
    fn test_retract() {
        assert_eq!(retract(1.0, 25.0), "G1 E-1.0000 F1500");
    }

    #[test]
    fn test_rapid_move() {
        assert_eq!(
            rapid_move(Some(10.0), Some(20.0), None),
            "G0 X10.0000 Y20.0000"
        );
    }

    #[test]
    fn test_linear_move() {
        assert_eq!(
            linear_move(Some(1.0), Some(2.0), Some(0.3), Some(0.5), Some(30.0)),
            "G1 X1.0000 Y2.0000 Z0.3000 E0.5000 F30"
        );
    }

    #[test]
    fn test_extruder_pneumatic() {
        let cmds = extruder_command(&ExtruderType::Pneumatic, 100.0, 1.0, 10.0);
        assert_eq!(cmds.len(), 2);
        // M300 with timed release (extrusion time + 100ms buffer)
        assert!(cmds[0].starts_with("M300 S100"));
        assert!(cmds[1].starts_with("G1 E1.0"));
    }

    #[test]
    fn test_extruder_screw() {
        let cmds = extruder_command(&ExtruderType::Screw, 0.0, 1.0, 10.0);
        assert_eq!(cmds.len(), 1);
        assert!(cmds[0].starts_with("G1 E1.0"));
    }
}
