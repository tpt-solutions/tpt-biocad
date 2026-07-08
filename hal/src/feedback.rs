// Real-time pressure/temperature feedback loop
// Licensed under Apache 2.0
//
// Closed-loop controller that reads printer sensor data and adjusts
// print parameters mid-print to maintain target pressure/temperature.

use std::collections::VecDeque;

/// Sensor reading from the printer.
#[derive(Debug, Clone, Copy)]
pub struct SensorReading {
    /// Measured pressure (kPa).
    pub pressure_kpa: f64,
    /// Measured temperature (°C).
    pub temperature_c: f64,
    /// Timestamp of the reading (seconds from start).
    pub timestamp_s: f64,
}

/// Target values for the feedback loop.
#[derive(Debug, Clone)]
pub struct ControlTarget {
    /// Target pressure (kPa).
    pub pressure_kpa: f64,
    /// Target temperature (°C).
    pub temperature_c: f64,
    /// Tolerance band for pressure (kPa). Within ±tolerance, no correction.
    pub pressure_tolerance: f64,
    /// Tolerance band for temperature (°C).
    pub temperature_tolerance: f64,
}

impl Default for ControlTarget {
    fn default() -> Self {
        Self {
            pressure_kpa: 100.0,
            temperature_c: 37.0,
            pressure_tolerance: 5.0,
            temperature_tolerance: 1.0,
        }
    }
}

/// PID controller gains.
#[derive(Debug, Clone)]
pub struct PidGains {
    pub kp: f64,
    pub ki: f64,
    pub kd: f64,
}

impl Default for PidGains {
    fn default() -> Self {
        Self {
            kp: 1.0,
            ki: 0.1,
            kd: 0.05,
        }
    }
}

/// Corrective action produced by the feedback controller.
#[derive(Debug, Clone)]
pub struct ControlAction {
    /// Recommended pressure adjustment (kPa change). Positive = increase.
    pub pressure_delta: f64,
    /// Recommended temperature adjustment (°C change). Positive = increase.
    pub temperature_delta: f64,
    /// Recommended print speed multiplier (1.0 = no change, <1 = slow down).
    pub speed_multiplier: f64,
    /// Whether the controller signals an abort condition.
    pub abort: bool,
    /// Human-readable explanation.
    pub message: Option<String>,
}

/// Stateful feedback controller for pressure and temperature regulation.
pub struct FeedbackController {
    /// PID gains for pressure loop.
    pub pressure_gains: PidGains,
    /// PID gains for temperature loop.
    pub temperature_gains: PidGains,
    /// Control target.
    pub target: ControlTarget,
    /// Window of recent sensor readings for derivative calculation.
    history: VecDeque<SensorReading>,
    /// Maximum history length.
    max_history: usize,
    /// Integrated error for pressure I-term.
    pressure_integral: f64,
    /// Integrated error for temperature I-term.
    temperature_integral: f64,
    /// Previous error values for D-term.
    prev_pressure_error: f64,
    prev_temperature_error: f64,
    /// Whether the controller has been initialized with a first reading.
    initialized: bool,
}

impl FeedbackController {
    /// Create a new feedback controller with default gains and target.
    pub fn new() -> Self {
        Self {
            pressure_gains: PidGains::default(),
            temperature_gains: PidGains::default(),
            target: ControlTarget::default(),
            history: VecDeque::new(),
            max_history: 100,
            pressure_integral: 0.0,
            temperature_integral: 0.0,
            prev_pressure_error: 0.0,
            prev_temperature_error: 0.0,
            initialized: false,
        }
    }

    /// Create a feedback controller with custom gains and target.
    pub fn with_params(
        pressure_gains: PidGains,
        temperature_gains: PidGains,
        target: ControlTarget,
    ) -> Self {
        Self {
            pressure_gains,
            temperature_gains,
            target,
            history: VecDeque::new(),
            max_history: 100,
            pressure_integral: 0.0,
            temperature_integral: 0.0,
            prev_pressure_error: 0.0,
            prev_temperature_error: 0.0,
            initialized: false,
        }
    }

    /// Feed a new sensor reading into the controller and compute corrective
    /// action.
    pub fn update(&mut self, reading: SensorReading) -> ControlAction {
        self.history.push_back(reading);
        if self.history.len() > self.max_history {
            self.history.pop_front();
        }

        if !self.initialized {
            self.prev_pressure_error = self.target.pressure_kpa - reading.pressure_kpa;
            self.prev_temperature_error = self.target.temperature_c - reading.temperature_c;
            self.initialized = true;
            return ControlAction {
                pressure_delta: 0.0,
                temperature_delta: 0.0,
                speed_multiplier: 1.0,
                abort: false,
                message: Some("Controller initialized".to_string()),
            };
        }

        let dt = self.dt();
        let pressure_error = self.target.pressure_kpa - reading.pressure_kpa;
        let temperature_error = self.target.temperature_c - reading.temperature_c;

        // Integral terms with anti-windup clamping.
        self.pressure_integral += pressure_error * dt;
        self.temperature_integral += temperature_error * dt;
        self.clamp_integrals();

        // Derivative terms (rate of change of error).
        let pressure_derivative = if dt > 0.0 {
            (pressure_error - self.prev_pressure_error) / dt
        } else {
            0.0
        };
        let temperature_derivative = if dt > 0.0 {
            (temperature_error - self.prev_temperature_error) / dt
        } else {
            0.0
        };

        // PID output.
        let pressure_output = self.pressure_gains.kp * pressure_error
            + self.pressure_gains.ki * self.pressure_integral
            + self.pressure_gains.kd * pressure_derivative;

        let temperature_output = self.temperature_gains.kp * temperature_error
            + self.temperature_gains.ki * self.temperature_integral
            + self.temperature_gains.kd * temperature_derivative;

        // Store current error for next iteration.
        self.prev_pressure_error = pressure_error;
        self.prev_temperature_error = temperature_error;

        // Deadband: within tolerance, no correction.
        let pressure_delta = if pressure_error.abs() > self.target.pressure_tolerance {
            pressure_output
        } else {
            0.0
        };

        let temperature_delta = if temperature_error.abs() > self.target.temperature_tolerance {
            temperature_output
        } else {
            0.0
        };

        // Speed adjustment: if pressure is too high (> +2× tolerance), slow down.
        let speed_multiplier = if pressure_error < -2.0 * self.target.pressure_tolerance {
            (1.0 + pressure_error / (self.target.pressure_kpa * 2.0)).clamp(0.3, 1.0)
        } else {
            1.0
        };

        // Abort detection: sustained large errors.
        let abort = pressure_error.abs() > self.target.pressure_kpa * 0.5
            && self.pressure_integral.abs() > self.target.pressure_kpa * 2.0;

        let message = if abort {
            Some(format!(
                "ABORT: pressure error {:.1}kPa (>{:.0}% of target), integral winding",
                pressure_error,
                self.target.pressure_kpa * 0.5,
            ))
        } else if pressure_delta.abs() > self.target.pressure_tolerance
            || temperature_delta.abs() > self.target.temperature_tolerance
        {
            Some(format!(
                "correcting: ΔP={:+.1}kPa, ΔT={:+.1}°C, speed={:.0}%",
                pressure_delta,
                temperature_delta,
                speed_multiplier * 100.0,
            ))
        } else {
            None
        };

        ControlAction {
            pressure_delta,
            temperature_delta,
            speed_multiplier,
            abort,
            message,
        }
    }

    /// Reset the controller state (e.g., for a new print job).
    pub fn reset(&mut self) {
        self.history.clear();
        self.pressure_integral = 0.0;
        self.temperature_integral = 0.0;
        self.prev_pressure_error = 0.0;
        self.prev_temperature_error = 0.0;
        self.initialized = false;
    }

    /// Recent history of sensor readings.
    pub fn history(&self) -> &VecDeque<SensorReading> {
        &self.history
    }

    /// Current pressure error.
    pub fn pressure_error(&self) -> f64 {
        self.history
            .back()
            .map(|r| self.target.pressure_kpa - r.pressure_kpa)
            .unwrap_or(0.0)
    }

    /// Current temperature error.
    pub fn temperature_error(&self) -> f64 {
        self.history
            .back()
            .map(|r| self.target.temperature_c - r.temperature_c)
            .unwrap_or(0.0)
    }

    fn dt(&self) -> f64 {
        if self.history.len() < 2 {
            return 0.1;
        }
        let prev = self.history[self.history.len() - 2];
        let curr = self.history.back().unwrap();
        (curr.timestamp_s - prev.timestamp_s).max(0.001)
    }

    fn clamp_integrals(&mut self) {
        let max_int = 500.0;
        self.pressure_integral = self.pressure_integral.clamp(-max_int, max_int);
        self.temperature_integral = self.temperature_integral.clamp(-max_int, max_int);
    }
}

impl Default for FeedbackController {
    fn default() -> Self {
        Self::new()
    }
}

/// Analyze a sequence of sensor readings to detect anomalies.
pub fn detect_sensor_anomaly(readings: &[SensorReading]) -> Option<String> {
    if readings.len() < 3 {
        return None;
    }

    let pressures: Vec<f64> = readings.iter().map(|r| r.pressure_kpa).collect();
    let temps: Vec<f64> = readings.iter().map(|r| r.temperature_c).collect();

    // Sudden pressure spike.
    let p_mean = pressures.iter().sum::<f64>() / pressures.len() as f64;
    for &p in &pressures {
        if (p - p_mean).abs() > p_mean * 0.8 && p_mean > 1.0 {
            return Some(format!(
                "pressure spike detected: {:.1}kPa vs mean {:.1}kPa",
                p, p_mean
            ));
        }
    }

    // Temperature runaway.
    let t_first = temps[0];
    let t_last = temps[temps.len() - 1];
    if (t_last - t_first).abs() > 10.0 {
        return Some(format!(
            "temperature drift: {:.1}°C → {:.1}°C (Δ={:+.1}°C)",
            t_first,
            t_last,
            t_last - t_first,
        ));
    }

    // Pressure oscillation (rapid up-down pattern).
    let mut sign_changes = 0;
    for i in 1..pressures.len() - 1 {
        let d1 = pressures[i] - pressures[i - 1];
        let d2 = pressures[i + 1] - pressures[i];
        if d1 * d2 < 0.0 && d1.abs() > 5.0 {
            sign_changes += 1;
        }
    }
    if sign_changes > readings.len() / 3 {
        return Some(format!(
            "pressure oscillation detected: {} sign changes in {} readings",
            sign_changes,
            readings.len(),
        ));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_initializes_without_action() {
        let mut ctrl = FeedbackController::new();
        let action = ctrl.update(SensorReading {
            pressure_kpa: 100.0,
            temperature_c: 37.0,
            timestamp_s: 0.0,
        });
        assert_eq!(action.pressure_delta, 0.0);
        assert_eq!(action.temperature_delta, 0.0);
    }

    #[test]
    fn test_controller_corrects_pressure_error() {
        let mut ctrl = FeedbackController::with_params(
            PidGains {
                kp: 2.0,
                ki: 0.0,
                kd: 0.0,
            },
            PidGains::default(),
            ControlTarget {
                pressure_kpa: 100.0,
                ..Default::default()
            },
        );

        // First reading: initializes.
        ctrl.update(SensorReading {
            pressure_kpa: 50.0,
            temperature_c: 37.0,
            timestamp_s: 0.0,
        });

        // Second reading: should compute a correction.
        let action = ctrl.update(SensorReading {
            pressure_kpa: 50.0,
            temperature_c: 37.0,
            timestamp_s: 1.0,
        });
        assert!(action.pressure_delta > 0.0);
    }

    #[test]
    fn test_controller_no_action_within_tolerance() {
        let mut ctrl = FeedbackController::with_params(
            PidGains {
                kp: 10.0,
                ki: 0.0,
                kd: 0.0,
            },
            PidGains::default(),
            ControlTarget {
                pressure_kpa: 100.0,
                pressure_tolerance: 10.0,
                ..Default::default()
            },
        );

        ctrl.update(SensorReading {
            pressure_kpa: 95.0,
            temperature_c: 37.0,
            timestamp_s: 0.0,
        });

        let action = ctrl.update(SensorReading {
            pressure_kpa: 95.0,
            temperature_c: 37.0,
            timestamp_s: 1.0,
        });
        assert_eq!(action.pressure_delta, 0.0);
    }

    #[test]
    fn test_speed_reduction_on_high_pressure() {
        let mut ctrl = FeedbackController::with_params(
            PidGains::default(),
            PidGains::default(),
            ControlTarget {
                pressure_kpa: 100.0,
                pressure_tolerance: 5.0,
                ..Default::default()
            },
        );

        ctrl.update(SensorReading {
            pressure_kpa: 100.0,
            temperature_c: 37.0,
            timestamp_s: 0.0,
        });

        // Pressure way above target.
        let action = ctrl.update(SensorReading {
            pressure_kpa: 150.0,
            temperature_c: 37.0,
            timestamp_s: 1.0,
        });
        assert!(action.speed_multiplier < 1.0);
    }

    #[test]
    fn test_reset_clears_state() {
        let mut ctrl = FeedbackController::new();
        ctrl.update(SensorReading {
            pressure_kpa: 50.0,
            temperature_c: 25.0,
            timestamp_s: 0.0,
        });

        assert!(ctrl.initialized);
        ctrl.reset();
        assert!(!ctrl.initialized);
        assert!(ctrl.history.is_empty());
    }

    #[test]
    fn test_detect_pressure_spike() {
        let readings = vec![
            SensorReading {
                pressure_kpa: 100.0,
                temperature_c: 37.0,
                timestamp_s: 0.0,
            },
            SensorReading {
                pressure_kpa: 110.0,
                temperature_c: 37.0,
                timestamp_s: 1.0,
            },
            SensorReading {
                pressure_kpa: 300.0,
                temperature_c: 37.0,
                timestamp_s: 2.0,
            },
            SensorReading {
                pressure_kpa: 105.0,
                temperature_c: 37.0,
                timestamp_s: 3.0,
            },
        ];
        let anomaly = detect_sensor_anomaly(&readings);
        assert!(anomaly.is_some());
        assert!(anomaly.unwrap().contains("spike"));
    }

    #[test]
    fn test_detect_temperature_drift() {
        let readings = vec![
            SensorReading {
                pressure_kpa: 100.0,
                temperature_c: 25.0,
                timestamp_s: 0.0,
            },
            SensorReading {
                pressure_kpa: 100.0,
                temperature_c: 30.0,
                timestamp_s: 10.0,
            },
            SensorReading {
                pressure_kpa: 100.0,
                temperature_c: 38.0,
                timestamp_s: 20.0,
            },
        ];
        let anomaly = detect_sensor_anomaly(&readings);
        assert!(anomaly.is_some());
    }

    #[test]
    fn test_no_false_positive_on_stable_readings() {
        let readings = vec![
            SensorReading {
                pressure_kpa: 100.0,
                temperature_c: 37.0,
                timestamp_s: 0.0,
            },
            SensorReading {
                pressure_kpa: 101.0,
                temperature_c: 37.1,
                timestamp_s: 1.0,
            },
            SensorReading {
                pressure_kpa: 99.0,
                temperature_c: 36.9,
                timestamp_s: 2.0,
            },
        ];
        assert!(detect_sensor_anomaly(&readings).is_none());
    }
}
