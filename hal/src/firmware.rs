// Firmware-specific protocol handling for M300-M303 commands
// Licensed under Apache 2.0
//
// Provides firmware-aware G-code formatting and protocol adaptation for
// Klipper, Marlin, and RepRapFirmware. Firmware detection is automatic
// based on connection handshake responses.

/// Supported firmware types for bioprinting/food printing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareType {
    /// Klipper firmware (primary target).
    Klipper,
    /// Marlin/RepRap firmware (standard 3D printer firmware).
    Marlin,
    /// RepRapFirmware (Duet boards).
    RepRapFirmware,
}

impl FirmwareType {
    /// Detect firmware from a handshake response.
    /// Klipper responds with "// Klipper", Marlin with "start",
    /// RepRapFirmware with "RepRapFirmware".
    pub fn detect(handshake: &str) -> Self {
        let lower = handshake.to_lowercase();
        if lower.contains("klipper") {
            FirmwareType::Klipper
        } else if lower.contains("reprap") || lower.contains("duet") {
            FirmwareType::RepRapFirmware
        } else {
            FirmwareType::Marlin
        }
    }
}

/// Firmware-aware command formatter.
pub struct FirmwareFormatter {
    pub firmware: FirmwareType,
}

impl FirmwareFormatter {
    /// Create a new formatter for the specified firmware.
    pub fn new(firmware: FirmwareType) -> Self {
        Self { firmware }
    }

    /// Format M300 — Pneumatic pressure control.
    /// Marlin: M300 is standard beep command; we use M300 S.. P.. if custom handler installed.
    /// RepRap: Uses M98 P"0:/macros/tpt/pneumatic.g" with parameters.
    pub fn pneumatic_pressure(&self, pressure_kpa: f64, duration_ms: f64) -> String {
        match self.firmware {
            FirmwareType::Klipper | FirmwareType::Marlin => {
                format!("M300 S{:.0} P{:.0}", pressure_kpa, duration_ms)
            }
            FirmwareType::RepRapFirmware => {
                format!(
                    "M98 P\"0:/macros/tpt/pneumatic.g\" S{:.0} P{:.0}",
                    pressure_kpa, duration_ms
                )
            }
        }
    }

    /// Format M301 — Thermal profiling.
    pub fn thermal_profile(&self, temp_c: f64, ramp_rate: f64) -> String {
        match self.firmware {
            FirmwareType::Klipper | FirmwareType::Marlin => {
                format!("M301 T{:.1} R{:.2}", temp_c, ramp_rate)
            }
            FirmwareType::RepRapFirmware => {
                format!(
                    "M98 P\"0:/macros/tpt/thermal.g\" S{:.1} R{:.2}",
                    temp_c, ramp_rate
                )
            }
        }
    }

    /// Format M302 — UV curing.
    pub fn uv_cure(&self, intensity_w_m2: f64, duration_s: f64) -> String {
        match self.firmware {
            FirmwareType::Klipper | FirmwareType::Marlin => {
                format!("M302 U{:.0} T{:.0}", intensity_w_m2, duration_s)
            }
            FirmwareType::RepRapFirmware => {
                format!(
                    "M98 P\"0:/macros/tpt/uv.g\" S{:.0} T{:.0}",
                    intensity_w_m2, duration_s
                )
            }
        }
    }

    /// Format M303 — Coaxial cross-linker control.
    pub fn coaxial_flow(&self, state: Option<i32>) -> String {
        match self.firmware {
            FirmwareType::Klipper | FirmwareType::Marlin => match state {
                Some(val) => format!("M303 S{}", val),
                None => "M303".to_string(),
            },
            FirmwareType::RepRapFirmware => {
                let s = state.unwrap_or(-1);
                format!("M98 P\"0:/macros/tpt/coaxial.g\" S{}", s)
            }
        }
    }

    /// Format a G0 rapid move.
    pub fn rapid_move(&self, x: Option<f64>, y: Option<f64>, z: Option<f64>) -> String {
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

    /// Format a G1 linear move.
    pub fn linear_move(
        &self,
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
}

/// Firmware-specific configuration for Extended M-codes.
/// This struct captures the pin and parameter mapping needed by each firmware.
#[derive(Debug, Clone)]
pub struct FirmwareConfig {
    pub firmware: FirmwareType,
    /// Pneumatic pressure control: PWM pin name/number (Klipper/Marlin) or
    /// GPIO port (RepRapFirmware).
    pub pneumatic_pin: Option<String>,
    /// Maximum pressure (kPa) the regulator can deliver.
    pub pneumatic_max_kpa: f64,
    /// UV curing: PWM pin name/number.
    pub uv_pin: Option<String>,
    /// Maximum UV intensity (W/m²).
    pub uv_max_intensity: f64,
    /// Coaxial cross-linker: digital output pin name/number.
    pub coaxial_pin: Option<String>,
    /// Heater name/number for thermal profiling.
    pub heater: Option<String>,
}

impl Default for FirmwareConfig {
    fn default() -> Self {
        Self {
            firmware: FirmwareType::Klipper,
            pneumatic_pin: None,
            pneumatic_max_kpa: 500.0,
            uv_pin: None,
            uv_max_intensity: 100.0,
            coaxial_pin: None,
            heater: None,
        }
    }
}

impl FirmwareConfig {
    /// Generate the printer.cfg / Configuration_adv.h / config.g snippet
    /// for the selected firmware.
    pub fn generate_config_snippet(&self) -> String {
        match self.firmware {
            FirmwareType::Klipper => self.klipper_snippet(),
            FirmwareType::Marlin => self.marlin_snippet(),
            FirmwareType::RepRapFirmware => self.reprap_snippet(),
        }
    }

    fn klipper_snippet(&self) -> String {
        let mut lines = vec![
            "# Klipper configuration for TPT BioCAD extended M-codes".to_string(),
            "# Add a [tpt_biocad] section to printer.cfg".to_string(),
            "".to_string(),
            "[tpt_biocad]".to_string(),
        ];
        if let Some(ref pin) = self.pneumatic_pin {
            lines.push(format!("pneumatic_pin: {}", pin));
            lines.push(format!("pneumatic_max_kpa: {:.0}", self.pneumatic_max_kpa));
            lines.push("pneumatic_pwm_range: 1000".to_string());
        }
        if let Some(ref pin) = self.uv_pin {
            lines.push(format!("uv_pin: {}", pin));
            lines.push(format!("uv_max_intensity: {:.0}", self.uv_max_intensity));
            lines.push("uv_pwm_range: 255".to_string());
        }
        if let Some(ref pin) = self.coaxial_pin {
            lines.push(format!("coaxial_pin: {}", pin));
        }
        if let Some(ref heater) = self.heater {
            lines.push(format!("heater: {}", heater));
        }
        lines.join("\n")
    }

    fn marlin_snippet(&self) -> String {
        let mut lines = vec![
            "// Marlin configuration for TPT BioCAD extended M-codes".to_string(),
            "// Add to Configuration_adv.h".to_string(),
            "".to_string(),
        ];
        if let Some(ref pin) = self.pneumatic_pin {
            lines.push(format!("#define TPT_PNEUMATIC_PIN {}", pin));
            lines.push(format!(
                "#define TPT_PNEUMATIC_MAX_KPA {:.0}",
                self.pneumatic_max_kpa
            ));
        }
        if let Some(ref pin) = self.uv_pin {
            lines.push(format!("#define TPT_UV_PIN {}", pin));
            lines.push(format!(
                "#define TPT_UV_MAX_W_M2 {:.0}",
                self.uv_max_intensity
            ));
        }
        if let Some(ref pin) = self.coaxial_pin {
            lines.push(format!("#define TPT_COAXIAL_PIN {}", pin));
        }
        if let Some(ref heater) = self.heater {
            lines.push(format!("#define TPT_HEATER_PIN {}", heater));
        }
        lines.join("\n")
    }

    fn reprap_snippet(&self) -> String {
        let mut lines = vec![
            "; RepRapFirmware configuration for TPT BioCAD extended M-codes".to_string(),
            "; Add to config.g".to_string(),
            "".to_string(),
            "; Create pin aliases for TPT BioCAD hardware".to_string(),
        ];
        if let Some(ref pin) = self.pneumatic_pin {
            lines.push(format!("global tpt.pneumatic_pin = {}", pin));
            lines.push(format!(
                "global tpt.pneumatic_max_kpa = {:.0}",
                self.pneumatic_max_kpa
            ));
        }
        if let Some(ref pin) = self.uv_pin {
            lines.push(format!("global tpt.uv_pin = {}", pin));
            lines.push(format!(
                "global tpt.uv_max_intensity = {:.0}",
                self.uv_max_intensity
            ));
        }
        if let Some(ref pin) = self.coaxial_pin {
            lines.push(format!("global tpt.coaxial_pin = {}", pin));
        }
        if let Some(ref heater) = self.heater {
            lines.push(format!("global tpt.heater = {}", heater));
        }
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_klipper() {
        assert_eq!(
            FirmwareType::detect("// Klipper 0.12.0"),
            FirmwareType::Klipper
        );
    }

    #[test]
    fn test_detect_marlin() {
        assert_eq!(
            FirmwareType::detect("start\nGrbl 1.1f"),
            FirmwareType::Marlin
        );
    }

    #[test]
    fn test_detect_reprap() {
        assert_eq!(
            FirmwareType::detect("RepRapFirmware 3.5"),
            FirmwareType::RepRapFirmware
        );
        assert_eq!(FirmwareType::detect("Duet 3"), FirmwareType::RepRapFirmware);
    }

    #[test]
    fn test_klipper_m300() {
        let fmt = FirmwareFormatter::new(FirmwareType::Klipper);
        assert_eq!(fmt.pneumatic_pressure(100.0, 500.0), "M300 S100 P500");
    }

    #[test]
    fn test_marlin_m300() {
        let fmt = FirmwareFormatter::new(FirmwareType::Marlin);
        assert_eq!(fmt.pneumatic_pressure(100.0, 500.0), "M300 S100 P500");
    }

    #[test]
    fn test_reprap_m300() {
        let fmt = FirmwareFormatter::new(FirmwareType::RepRapFirmware);
        let cmd = fmt.pneumatic_pressure(100.0, 500.0);
        assert!(cmd.contains("M98"));
        assert!(cmd.contains("pneumatic"));
    }

    #[test]
    fn test_klipper_m301() {
        let fmt = FirmwareFormatter::new(FirmwareType::Klipper);
        assert_eq!(fmt.thermal_profile(31.0, 0.5), "M301 T31.0 R0.50");
    }

    #[test]
    fn test_uv_cure_formats() {
        let fmt = FirmwareFormatter::new(FirmwareType::Klipper);
        assert_eq!(fmt.uv_cure(10.0, 30.0), "M302 U10 T30");
    }

    #[test]
    fn test_coaxial_formats() {
        let fmt = FirmwareFormatter::new(FirmwareType::Klipper);
        assert_eq!(fmt.coaxial_flow(None), "M303");
        assert_eq!(fmt.coaxial_flow(Some(1)), "M303 S1");
    }

    #[test]
    fn test_config_snippet_klipper() {
        let cfg = FirmwareConfig {
            firmware: FirmwareType::Klipper,
            pneumatic_pin: Some("gpio20".into()),
            uv_pin: Some("gpio21".into()),
            coaxial_pin: Some("gpio22".into()),
            heater: Some("heater_bed".into()),
            ..Default::default()
        };
        let snippet = cfg.generate_config_snippet();
        assert!(snippet.contains("[tpt_biocad]"));
        assert!(snippet.contains("pneumatic_pin: gpio20"));
        assert!(snippet.contains("uv_pin: gpio21"));
    }

    #[test]
    fn test_config_snippet_marlin() {
        let cfg = FirmwareConfig {
            firmware: FirmwareType::Marlin,
            pneumatic_pin: Some("PC3".into()),
            ..Default::default()
        };
        let snippet = cfg.generate_config_snippet();
        assert!(snippet.contains("TPT_PNEUMATIC_PIN PC3"));
    }

    #[test]
    fn test_config_snippet_reprap() {
        let cfg = FirmwareConfig {
            firmware: FirmwareType::RepRapFirmware,
            pneumatic_pin: Some("gpio20".into()),
            ..Default::default()
        };
        let snippet = cfg.generate_config_snippet();
        assert!(snippet.contains("global tpt.pneumatic_pin"));
    }

    #[test]
    fn test_marlin_rapid_move() {
        let fmt = FirmwareFormatter::new(FirmwareType::Marlin);
        assert_eq!(
            fmt.rapid_move(Some(10.0), Some(20.0), None),
            "G0 X10.0000 Y20.0000"
        );
    }

    #[test]
    fn test_reprap_coaxial() {
        let fmt = FirmwareFormatter::new(FirmwareType::RepRapFirmware);
        let cmd = fmt.coaxial_flow(Some(1));
        assert!(cmd.contains("coaxial"));
        assert!(cmd.contains("S1"));
    }
}
