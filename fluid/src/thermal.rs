// Thermal profiling algorithms (chocolate tempering, etc.)
// Licensed under Apache 2.0
//
// Models temperature ramp profiles used for chocolate tempering and other
// phase-change-sensitive food materials. A tempering curve is a sequence of
// (temperature, hold_time) segments that the printer's heater should follow to
// achieve a stable crystal form.

use serde::{Deserialize, Serialize};

/// A single segment of a thermal profile: hold at `temperature` (°C) for
/// `hold_time` (s), optionally ramping from the previous segment at
/// `ramp_rate` (°C/s).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalSegment {
    pub temperature: f64, // °C
    pub hold_time: f64,   // s
    pub ramp_rate: f64,   // °C/s (0 = instantaneous)
}

/// A named thermal profile (e.g. a dark-chocolate tempering curve).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThermalProfile {
    pub name: String,
    pub segments: Vec<ThermalSegment>,
}

impl ThermalProfile {
    /// Total duration of the profile in seconds.
    pub fn total_time(&self) -> f64 {
        let mut prev_temp = 0.0;
        let mut total = 0.0;
        for s in &self.segments {
            let ramp_time = if s.ramp_rate > 0.0 {
                (s.temperature - prev_temp).abs() / s.ramp_rate
            } else {
                0.0
            };
            total += ramp_time + s.hold_time;
            prev_temp = s.temperature;
        }
        total
    }

    /// Peak temperature reached by the profile (°C).
    pub fn peak_temperature(&self) -> f64 {
        self.segments
            .iter()
            .map(|s| s.temperature)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Sample the target temperature at time `t` (s) into the profile.
    /// Interpolates linearly during ramp phases.
    pub fn temperature_at(&self, t: f64) -> f64 {
        let mut elapsed = 0.0;
        let mut prev_temp = 0.0;
        for seg in &self.segments {
            let ramp_time = if seg.ramp_rate > 0.0 {
                (seg.temperature - prev_temp).abs() / seg.ramp_rate
            } else {
                0.0
            };

            // During ramp phase: interpolate from prev_temp to seg.temperature
            if ramp_time > 0.0 && t <= elapsed + ramp_time {
                let frac = (t - elapsed) / ramp_time;
                return prev_temp + frac * (seg.temperature - prev_temp);
            }
            elapsed += ramp_time;

            // During hold phase: constant at seg.temperature
            if t <= elapsed + seg.hold_time {
                return seg.temperature;
            }
            elapsed += seg.hold_time;

            prev_temp = seg.temperature;
        }
        // Past the end: hold final temperature.
        self.segments.last().map(|s| s.temperature).unwrap_or(0.0)
    }
}

/// Built-in dark chocolate tempering profile (°C), based on the standard
/// melt → cool → re-warm → hold sequence that stabilises Form V crystals.
pub fn dark_chocolate_tempering() -> ThermalProfile {
    ThermalProfile {
        name: "Dark Chocolate Tempering".to_string(),
        segments: vec![
            ThermalSegment {
                temperature: 45.0,
                hold_time: 60.0,
                ramp_rate: 2.0,
            }, // melt
            ThermalSegment {
                temperature: 27.0,
                hold_time: 90.0,
                ramp_rate: 1.5,
            }, // cool / seed
            ThermalSegment {
                temperature: 31.0,
                hold_time: 120.0,
                ramp_rate: 0.5,
            }, // re-warm / hold
        ],
    }
}

/// Built-in milk chocolate tempering profile (°C) — lower peak temperature
/// because milk solids scorch more easily.
pub fn milk_chocolate_tempering() -> ThermalProfile {
    ThermalProfile {
        name: "Milk Chocolate Tempering".to_string(),
        segments: vec![
            ThermalSegment {
                temperature: 40.0,
                hold_time: 60.0,
                ramp_rate: 2.0,
            },
            ThermalSegment {
                temperature: 25.0,
                hold_time: 90.0,
                ramp_rate: 1.5,
            },
            ThermalSegment {
                temperature: 29.0,
                hold_time: 120.0,
                ramp_rate: 0.5,
            },
        ],
    }
}

/// Convert a ThermalProfile to serializable ThermalProfileData.
pub fn to_profile_data(profile: &ThermalProfile) -> tpt_core::ThermalProfileData {
    tpt_core::ThermalProfileData {
        name: profile.name.clone(),
        segments: profile
            .segments
            .iter()
            .map(|s| tpt_core::ThermalSegmentData {
                temperature: s.temperature,
                hold_time: s.hold_time,
                ramp_rate: s.ramp_rate,
            })
            .collect(),
    }
}

/// Convert serializable ThermalProfileData back to a ThermalProfile.
pub fn from_profile_data(data: &tpt_core::ThermalProfileData) -> ThermalProfile {
    ThermalProfile {
        name: data.name.clone(),
        segments: data
            .segments
            .iter()
            .map(|s| ThermalSegment {
                temperature: s.temperature,
                hold_time: s.hold_time,
                ramp_rate: s.ramp_rate,
            })
            .collect(),
    }
}

/// Generate a G-code style list of M301 (thermal profiling) commands for the
/// given profile, suitable for the Klipper M301 extension described in the spec.
pub fn profile_to_m301(profile: &ThermalProfile) -> Vec<String> {
    profile
        .segments
        .iter()
        .map(|s| format!("M301 T{:.1} R{:.2}", s.temperature, s.ramp_rate))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_chocolate_profile() {
        let p = dark_chocolate_tempering();
        assert!(p.peak_temperature() > 40.0);
        assert!(p.total_time() > 0.0);
        // At t=0 we start ramping from 0 toward the first segment (45°C).
        // With ramp_rate=2.0, at t=0 we should be at 0 (start of ramp from 0).
        assert_eq!(p.temperature_at(0.0), 0.0);
        // After the first ramp completes (45/2 = 22.5s), should be at 45°C.
        assert_eq!(p.temperature_at(22.5), 45.0);
        // Well past the end, holds final temperature.
        assert_eq!(p.temperature_at(p.total_time() + 100.0), 31.0);
    }

    #[test]
    fn test_m301_generation() {
        let p = dark_chocolate_tempering();
        let cmds = profile_to_m301(&p);
        assert_eq!(cmds.len(), 3);
        assert!(cmds[0].starts_with("M301 T45.0"));
    }

    #[test]
    fn test_thermal_profile_roundtrip() {
        let original = dark_chocolate_tempering();
        let data = to_profile_data(&original);
        let restored = from_profile_data(&data);
        assert_eq!(original.name, restored.name);
        assert_eq!(original.segments.len(), restored.segments.len());
        for (a, b) in original.segments.iter().zip(restored.segments.iter()) {
            assert_eq!(a.temperature, b.temperature);
            assert_eq!(a.hold_time, b.hold_time);
            assert_eq!(a.ramp_rate, b.ramp_rate);
        }
    }

    #[test]
    fn test_thermal_profile_json_roundtrip() {
        let original = milk_chocolate_tempering();
        let data = to_profile_data(&original);
        let json = serde_json::to_string(&data).unwrap();
        let restored_data: tpt_core::ThermalProfileData = serde_json::from_str(&json).unwrap();
        let restored = from_profile_data(&restored_data);
        assert_eq!(original.segments.len(), restored.segments.len());
    }
}
