// Thermal profiling algorithms (chocolate tempering, etc.)
// Licensed under Apache 2.0
//
// Models temperature ramp profiles used for chocolate tempering and other
// phase-change-sensitive food materials. A tempering curve is a sequence of
// (temperature, hold_time) segments that the printer's heater should follow to
// achieve a stable crystal form.

/// A single segment of a thermal profile: hold at `temperature` (°C) for
/// `hold_time` (s), optionally ramping from the previous segment at
/// `ramp_rate` (°C/s).
#[derive(Debug, Clone)]
pub struct ThermalSegment {
    pub temperature: f64, // °C
    pub hold_time: f64,   // s
    pub ramp_rate: f64,   // °C/s (0 = instantaneous)
}

/// A named thermal profile (e.g. a dark-chocolate tempering curve).
#[derive(Debug, Clone)]
pub struct ThermalProfile {
    pub name: String,
    pub segments: Vec<ThermalSegment>,
}

impl ThermalProfile {
    /// Total duration of the profile in seconds.
    pub fn total_time(&self) -> f64 {
        self.segments
            .iter()
            .map(|s| {
                s.hold_time
                    + if s.ramp_rate > 0.0 {
                        s.temperature.abs() / s.ramp_rate
                    } else {
                        0.0
                    }
            })
            .sum()
    }

    /// Peak temperature reached by the profile (°C).
    pub fn peak_temperature(&self) -> f64 {
        self.segments
            .iter()
            .map(|s| s.temperature)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Sample the target temperature at time `t` (s) into the profile.
    /// Returns the segment temperature once the ramp (if any) is complete.
    pub fn temperature_at(&self, t: f64) -> f64 {
        let mut elapsed = 0.0;
        for seg in &self.segments {
            let ramp_time = if seg.ramp_rate > 0.0 {
                seg.temperature.abs() / seg.ramp_rate
            } else {
                0.0
            };
            let seg_duration = ramp_time + seg.hold_time;
            if t <= elapsed + seg_duration {
                return seg.temperature;
            }
            elapsed += seg_duration;
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
        // At t=0 we are ramping toward the first segment temperature.
        assert_eq!(p.temperature_at(0.0), 45.0);
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
}
