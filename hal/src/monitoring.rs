// Quality monitoring: detect under-extrusion, stringing, layer shifts
// Licensed under Apache 2.0
//
// Analyses sensor data (pressure, temperature, position, flow) to detect
// common 3D printing defects in real time.

use std::collections::VecDeque;

/// A sensor sample from the printer during a print.
#[derive(Debug, Clone, Copy)]
pub struct PrintSample {
    /// Extruder pressure (kPa).
    pub pressure_kpa: f64,
    /// Extruder temperature (°C).
    pub temperature_c: f64,
    /// Current flow rate (mm³/s).
    pub flow_rate: f64,
    /// Current X position (mm).
    pub x: f64,
    /// Current Y position (mm).
    pub y: f64,
    /// Current Z position (mm).
    pub z: f64,
    /// Current extruded volume since start (mm³).
    pub extruded_volume: f64,
    /// Timestamp (seconds from start).
    pub timestamp_s: f64,
}

/// Quality alert severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}

/// A quality alert produced by the monitoring system.
#[derive(Debug, Clone)]
pub struct QualityAlert {
    /// Type of defect detected.
    pub defect_type: DefectType,
    /// Severity level.
    pub severity: AlertSeverity,
    /// Human-readable description.
    pub message: String,
    /// Timestamp when the defect was detected.
    pub timestamp_s: f64,
    /// Location where the defect was detected.
    pub location: Option<(f64, f64, f64)>,
}

/// Type of print defect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefectType {
    UnderExtrusion,
    OverExtrusion,
    Stringing,
    LayerShift,
    PressureSpike,
    TemperatureDrift,
    FlowInconsistency,
}

impl std::fmt::Display for DefectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DefectType::UnderExtrusion => write!(f, "under-extrusion"),
            DefectType::OverExtrusion => write!(f, "over-extrusion"),
            DefectType::Stringing => write!(f, "stringing"),
            DefectType::LayerShift => write!(f, "layer shift"),
            DefectType::PressureSpike => write!(f, "pressure spike"),
            DefectType::TemperatureDrift => write!(f, "temperature drift"),
            DefectType::FlowInconsistency => write!(f, "flow inconsistency"),
        }
    }
}

/// Configuration for the quality monitor.
#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// Expected flow rate (mm³/s) — used as baseline for extrusion detection.
    pub expected_flow_rate: f64,
    /// Fraction of expected flow below which under-extrusion is flagged.
    pub under_extrusion_threshold: f64,
    /// Fraction of expected flow above which over-extrusion is flagged.
    pub over_extrusion_threshold: f64,
    /// Maximum allowed deviation in Z per layer (mm). Larger = shift.
    pub max_layer_shift_mm: f64,
    /// Maximum allowed pressure variation (kPa) before flagging.
    pub max_pressure_variation_kpa: f64,
    /// Maximum allowed temperature drift (°C) over the monitoring window.
    pub max_temperature_drift_c: f64,
    /// Window size for moving statistics.
    pub window_size: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            expected_flow_rate: 1.0,
            under_extrusion_threshold: 0.7,
            over_extrusion_threshold: 1.3,
            max_layer_shift_mm: 0.5,
            max_pressure_variation_kpa: 20.0,
            max_temperature_drift_c: 3.0,
            window_size: 50,
        }
    }
}

/// Real-time quality monitoring system.
pub struct QualityMonitor {
    config: MonitorConfig,
    /// Ring buffer of recent samples.
    samples: VecDeque<PrintSample>,
    /// Generated alerts.
    alerts: Vec<QualityAlert>,
    /// Last known Z position per layer for shift detection.
    last_layer_z: Option<f64>,
    /// Cumulative extruded volume expectation based on ideal flow.
    expected_volume: f64,
    /// Time of last sample for flow calculation.
    last_sample_time: Option<f64>,
}

impl QualityMonitor {
    /// Create a new quality monitor with default configuration.
    pub fn new(config: MonitorConfig) -> Self {
        Self {
            config,
            samples: VecDeque::new(),
            alerts: Vec::new(),
            last_layer_z: None,
            expected_volume: 0.0,
            last_sample_time: None,
        }
    }

    /// Feed a new sensor sample into the monitor.
    pub fn feed(&mut self, sample: PrintSample) -> Vec<QualityAlert> {
        let mut new_alerts = Vec::new();

        self.samples.push_back(sample);
        if self.samples.len() > self.config.window_size {
            self.samples.pop_front();
        }

        let dt = match self.last_sample_time {
            Some(t) => (sample.timestamp_s - t).max(0.0),
            None => 0.0,
        };
        self.last_sample_time = Some(sample.timestamp_s);

        // Track expected volume.
        self.expected_volume += self.config.expected_flow_rate * dt;

        // Run all detectors.
        if let Some(alert) = self.detect_extrusion_issue(&sample) {
            new_alerts.push(alert);
        }
        if let Some(alert) = self.detect_stringing(&sample) {
            new_alerts.push(alert);
        }
        if let Some(alert) = self.detect_layer_shift(&sample) {
            new_alerts.push(alert);
        }
        if let Some(alert) = self.detect_pressure_issue(&sample) {
            new_alerts.push(alert);
        }
        if let Some(alert) = self.detect_temperature_drift() {
            new_alerts.push(alert);
        }

        self.alerts.extend(new_alerts.iter().cloned());

        // Trim old alerts.
        if self.alerts.len() > 500 {
            self.alerts.drain(0..self.alerts.len() - 500);
        }

        new_alerts
    }

    /// Get all alerts generated so far.
    pub fn alerts(&self) -> &[QualityAlert] {
        &self.alerts
    }

    /// Get mutable reference to the monitor configuration.
    pub fn config_mut(&mut self) -> &mut MonitorConfig {
        &mut self.config
    }

    /// Clear all alerts.
    pub fn clear_alerts(&mut self) {
        self.alerts.clear();
    }

    /// Reset the monitor state (for a new print).
    pub fn reset(&mut self) {
        self.samples.clear();
        self.alerts.clear();
        self.last_layer_z = None;
        self.expected_volume = 0.0;
        self.last_sample_time = None;
    }

    /// Get the current defect rate (alerts per minute).
    pub fn alert_rate(&self, current_time_s: f64) -> f64 {
        if current_time_s <= 0.0 {
            return 0.0;
        }
        let recent = self
            .alerts
            .iter()
            .filter(|a| a.timestamp_s > current_time_s - 60.0)
            .count();
        recent as f64 / (current_time_s / 60.0).max(1.0)
    }

    /// Overall print quality score (0.0 = failed, 1.0 = perfect).
    pub fn quality_score(&self) -> f64 {
        if self.alerts.is_empty() {
            return 1.0;
        }
        let critical = self
            .alerts
            .iter()
            .filter(|a| a.severity == AlertSeverity::Critical)
            .count() as f64;
        let warnings = self
            .alerts
            .iter()
            .filter(|a| a.severity == AlertSeverity::Warning)
            .count() as f64;
        let info = self
            .alerts
            .iter()
            .filter(|a| a.severity == AlertSeverity::Info)
            .count() as f64;

        let score = 1.0 - (critical * 0.3 + warnings * 0.1 + info * 0.02).min(1.0);
        score.max(0.0)
    }

    // --- Detectors ---

    fn detect_extrusion_issue(&self, sample: &PrintSample) -> Option<QualityAlert> {
        if self.samples.len() < 5 {
            return None;
        }

        // Compare actual extruded volume vs expected.
        if self.expected_volume > 1.0 {
            let actual_ratio = sample.extruded_volume / self.expected_volume;

            if actual_ratio < self.config.under_extrusion_threshold {
                return Some(QualityAlert {
                    defect_type: DefectType::UnderExtrusion,
                    severity: if actual_ratio < 0.5 {
                        AlertSeverity::Critical
                    } else {
                        AlertSeverity::Warning
                    },
                    message: format!(
                        "under-extrusion: actual/expected flow = {:.0}% (threshold {:.0}%)",
                        actual_ratio * 100.0,
                        self.config.under_extrusion_threshold * 100.0,
                    ),
                    timestamp_s: sample.timestamp_s,
                    location: Some((sample.x, sample.y, sample.z)),
                });
            }

            if actual_ratio > self.config.over_extrusion_threshold {
                return Some(QualityAlert {
                    defect_type: DefectType::OverExtrusion,
                    severity: AlertSeverity::Warning,
                    message: format!(
                        "over-extrusion: actual/expected flow = {:.0}% (threshold {:.0}%)",
                        actual_ratio * 100.0,
                        self.config.over_extrusion_threshold * 100.0,
                    ),
                    timestamp_s: sample.timestamp_s,
                    location: Some((sample.x, sample.y, sample.z)),
                });
            }
        }

        // Flow rate consistency: look for sudden drops.
        if self.samples.len() >= 3 {
            let recent: Vec<f64> = self
                .samples
                .iter()
                .rev()
                .take(3)
                .map(|s| s.flow_rate)
                .collect();
            let mean = recent.iter().sum::<f64>() / recent.len() as f64;
            if mean > 0.0 && sample.flow_rate < mean * 0.3 {
                return Some(QualityAlert {
                    defect_type: DefectType::FlowInconsistency,
                    severity: AlertSeverity::Warning,
                    message: format!(
                        "flow drop: {:.2}mm³/s vs recent mean {:.2}mm³/s",
                        sample.flow_rate, mean,
                    ),
                    timestamp_s: sample.timestamp_s,
                    location: Some((sample.x, sample.y, sample.z)),
                });
            }
        }

        None
    }

    fn detect_stringing(&self, _sample: &PrintSample) -> Option<QualityAlert> {
        // Stringing detection: look for extrusion during rapid moves.
        // This is a simplified heuristic: if flow is non-zero while the head
        // is moving fast (> 20mm/s) and not in a print path, it's stringing.
        //
        // In a real system, this would compare against the expected toolpath.
        // Here we flag when pressure is elevated during Z-hops or travel moves.
        if self.samples.len() < 10 {
            return None;
        }

        let recent: Vec<&PrintSample> = self.samples.iter().rev().take(10).collect();
        let has_pressure_during_travel = recent.windows(2).any(|w| {
            let dx = w[1].x - w[0].x;
            let dy = w[1].y - w[0].y;
            let speed =
                (dx * dx + dy * dy).sqrt() / (w[1].timestamp_s - w[0].timestamp_s).max(0.001);
            speed > 20.0 && w[1].flow_rate > 0.01 && w[0].flow_rate < 0.001
        });

        if has_pressure_during_travel {
            return Some(QualityAlert {
                defect_type: DefectType::Stringing,
                severity: AlertSeverity::Warning,
                message: "potential stringing: flow detected during rapid travel move".to_string(),
                timestamp_s: recent[0].timestamp_s,
                location: Some((recent[0].x, recent[0].y, recent[0].z)),
            });
        }

        None
    }

    fn detect_layer_shift(&mut self, sample: &PrintSample) -> Option<QualityAlert> {
        // Layer shift detection: sudden jump in X/Y during a non-travel move
        // that exceeds the configured threshold.
        if self.samples.len() < 2 {
            self.last_layer_z = Some(sample.z);
            return None;
        }

        let prev = self.samples[self.samples.len() - 2];

        // Only check when staying in the same layer (Z hasn't changed much).
        if (sample.z - prev.z).abs() > 0.01 {
            self.last_layer_z = Some(sample.z);
            return None;
        }

        // Large position change at constant Z = potential layer shift.
        let dx = (sample.x - prev.x).abs();
        let dy = (sample.y - prev.y).abs();
        let dt = (sample.timestamp_s - prev.timestamp_s).max(0.001);
        let speed = (dx + dy) / dt;

        // If moving fast and not along a normal path, flag it.
        if dx > self.config.max_layer_shift_mm || dy > self.config.max_layer_shift_mm {
            if speed > 50.0 {
                // Likely a rapid move, not a shift.
                return None;
            }
            return Some(QualityAlert {
                defect_type: DefectType::LayerShift,
                severity: AlertSeverity::Critical,
                message: format!(
                    "layer shift: X jumped {:.1}mm, Y jumped {:.1}mm at Z={:.2}",
                    dx, dy, sample.z,
                ),
                timestamp_s: sample.timestamp_s,
                location: Some((sample.x, sample.y, sample.z)),
            });
        }

        None
    }

    fn detect_pressure_issue(&self, sample: &PrintSample) -> Option<QualityAlert> {
        if self.samples.len() < 5 {
            return None;
        }

        let pressures: Vec<f64> = self.samples.iter().map(|s| s.pressure_kpa).collect();
        let mean = pressures.iter().sum::<f64>() / pressures.len() as f64;

        // Sudden spike.
        if sample.pressure_kpa > mean + self.config.max_pressure_variation_kpa {
            return Some(QualityAlert {
                defect_type: DefectType::PressureSpike,
                severity: AlertSeverity::Warning,
                message: format!(
                    "pressure spike: {:.0}kPa vs mean {:.0}kPa (Δ{:.0} > {:.0})",
                    sample.pressure_kpa,
                    mean,
                    sample.pressure_kpa - mean,
                    self.config.max_pressure_variation_kpa,
                ),
                timestamp_s: sample.timestamp_s,
                location: Some((sample.x, sample.y, sample.z)),
            });
        }

        // Pressure too low during extrusion.
        if sample.flow_rate > 0.1 && sample.pressure_kpa < 10.0 {
            return Some(QualityAlert {
                defect_type: DefectType::UnderExtrusion,
                severity: AlertSeverity::Critical,
                message: format!(
                    "pressure too low for extrusion: {:.0}kPa at flow {:.2}mm³/s",
                    sample.pressure_kpa, sample.flow_rate,
                ),
                timestamp_s: sample.timestamp_s,
                location: Some((sample.x, sample.y, sample.z)),
            });
        }

        None
    }

    fn detect_temperature_drift(&self) -> Option<QualityAlert> {
        if self.samples.len() < 10 {
            return None;
        }

        let temps: Vec<f64> = self.samples.iter().map(|s| s.temperature_c).collect();
        let first = temps[0];
        let last = temps[temps.len() - 1];
        let drift = (last - first).abs();

        if drift > self.config.max_temperature_drift_c {
            return Some(QualityAlert {
                defect_type: DefectType::TemperatureDrift,
                severity: AlertSeverity::Warning,
                message: format!(
                    "temperature drift: {:.1}°C over {} samples (max {:.0}°C)",
                    drift,
                    temps.len(),
                    self.config.max_temperature_drift_c,
                ),
                timestamp_s: self.samples.back().unwrap().timestamp_s,
                location: None,
            });
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(
        pressure: f64,
        temp: f64,
        flow: f64,
        x: f64,
        y: f64,
        z: f64,
        volume: f64,
        t: f64,
    ) -> PrintSample {
        PrintSample {
            pressure_kpa: pressure,
            temperature_c: temp,
            flow_rate: flow,
            x,
            y,
            z,
            extruded_volume: volume,
            timestamp_s: t,
        }
    }

    #[test]
    fn test_no_alerts_on_normal_print() {
        let config = MonitorConfig {
            expected_flow_rate: 1.0,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        for i in 0..20 {
            let alerts = monitor.feed(sample(
                100.0,
                37.0,
                1.0,
                i as f64 * 0.1,
                0.0,
                0.2,
                i as f64 * 0.1,
                i as f64 * 0.1,
            ));
            assert!(alerts.is_empty(), "unexpected alert at sample {}", i);
        }
    }

    #[test]
    fn test_under_extrusion_detected() {
        let config = MonitorConfig {
            expected_flow_rate: 1.0,
            under_extrusion_threshold: 0.7,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        // Print with very low actual extrusion.
        for i in 0..15 {
            monitor.feed(sample(
                100.0,
                37.0,
                0.1,
                0.0,
                0.0,
                0.2,
                i as f64 * 0.02,
                i as f64,
            ));
        }

        let alerts = monitor.alerts();
        let under_extrusion = alerts
            .iter()
            .any(|a| a.defect_type == DefectType::UnderExtrusion);
        assert!(under_extrusion, "should detect under-extrusion");
    }

    #[test]
    fn test_layer_shift_detected() {
        let config = MonitorConfig {
            max_layer_shift_mm: 0.5,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        // Normal moves.
        for i in 0..5 {
            monitor.feed(sample(100.0, 37.0, 0.5, i as f64, 0.0, 0.2, 0.5, i as f64));
        }

        // Large jump at same Z (layer shift).
        let alerts = monitor.feed(sample(100.0, 37.0, 0.5, 10.0, 0.0, 0.2, 0.5, 5.0));
        let shift = alerts
            .iter()
            .any(|a| a.defect_type == DefectType::LayerShift);
        assert!(shift, "should detect layer shift");
    }

    #[test]
    fn test_pressure_spike_detected() {
        let config = MonitorConfig {
            max_pressure_variation_kpa: 20.0,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        for i in 0..5 {
            monitor.feed(sample(100.0, 37.0, 1.0, 0.0, 0.0, 0.2, 1.0, i as f64));
        }

        let alerts = monitor.feed(sample(300.0, 37.0, 1.0, 0.0, 0.0, 0.2, 1.0, 5.0));
        let spike = alerts
            .iter()
            .any(|a| a.defect_type == DefectType::PressureSpike);
        assert!(spike);
    }

    #[test]
    fn test_temperature_drift_detected() {
        let config = MonitorConfig {
            max_temperature_drift_c: 3.0,
            window_size: 20,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        // Gradually increasing temperature.
        for i in 0..15 {
            monitor.feed(sample(
                100.0,
                30.0 + i as f64 * 0.5,
                1.0,
                0.0,
                0.0,
                0.2,
                1.0,
                i as f64,
            ));
        }

        let alerts = monitor.alerts();
        let drift = alerts
            .iter()
            .any(|a| a.defect_type == DefectType::TemperatureDrift);
        assert!(drift);
    }

    #[test]
    fn test_quality_score_perfect() {
        let config = MonitorConfig::default();
        let monitor = QualityMonitor::new(config);
        assert!((monitor.quality_score() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_quality_score_reduced_by_critical() {
        let config = MonitorConfig {
            max_layer_shift_mm: 0.1,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        monitor.feed(sample(100.0, 37.0, 1.0, 0.0, 0.0, 0.2, 1.0, 0.0));
        monitor.feed(sample(100.0, 37.0, 1.0, 50.0, 0.0, 0.2, 1.0, 1.0));

        assert!(monitor.quality_score() < 1.0);
    }

    #[test]
    fn test_reset_clears_state() {
        let config = MonitorConfig {
            max_pressure_variation_kpa: 1.0,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        // Feed enough samples to get a pressure spike.
        for i in 0..5 {
            monitor.feed(sample(100.0, 37.0, 1.0, 0.0, 0.0, 0.2, 1.0, i as f64));
        }
        monitor.feed(sample(300.0, 37.0, 1.0, 0.0, 0.0, 0.2, 1.0, 5.0));
        assert!(
            !monitor.alerts().is_empty(),
            "should have alerts from pressure spike"
        );

        monitor.reset();
        assert!(monitor.alerts().is_empty());
        assert!((monitor.quality_score() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_alert_rate() {
        let config = MonitorConfig {
            max_pressure_variation_kpa: 1.0,
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        // Feed enough samples to generate alerts.
        for i in 0..5 {
            monitor.feed(sample(100.0, 37.0, 1.0, 0.0, 0.0, 0.2, 1.0, i as f64));
        }
        monitor.feed(sample(300.0, 37.0, 1.0, 0.0, 0.0, 0.2, 1.0, 5.0));
        let rate = monitor.alert_rate(60.0);
        assert!(
            rate > 0.0,
            "alert rate should be > 0 after generating alerts"
        );
    }

    #[test]
    fn test_flow_inconsistency() {
        let config = MonitorConfig {
            expected_flow_rate: 1.0,
            under_extrusion_threshold: 0.1, // disable under-extrusion for this test
            ..Default::default()
        };
        let mut monitor = QualityMonitor::new(config);

        // Steady flow then sudden drop (cumulative volume).
        for i in 0..5 {
            monitor.feed(sample(
                100.0,
                37.0,
                1.0,
                0.0,
                0.0,
                0.2,
                (i + 1) as f64,
                i as f64,
            ));
        }
        let alerts = monitor.feed(sample(100.0, 37.0, 0.1, 0.0, 0.0, 0.2, 5.1, 5.0));
        let inc = alerts
            .iter()
            .any(|a| a.defect_type == DefectType::FlowInconsistency);
        assert!(inc);
    }
}
