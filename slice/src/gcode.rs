// G-code generation
// Licensed under Apache 2.0

use crate::slicer::ToolpathCommand;

/// G-code command
#[derive(Debug, Clone)]
pub enum GCodeCommand {
    G0 {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        e: Option<f64>,
    }, // Rapid move
    G1 {
        x: Option<f64>,
        y: Option<f64>,
        z: Option<f64>,
        e: Option<f64>,
        f: Option<f64>,
    }, // Linear move
    G28, // Home
    M104 {
        s: f64,
    }, // Set temperature
    M109 {
        s: f64,
    }, // Wait for temperature
    M300 {
        s: f64,
        p: f64,
    }, // Pneumatic pressure control
    M301 {
        t: f64,
        r: f64,
    }, // Thermal profiling
    M302 {
        u: f64,
        t: f64,
    }, // UV curing
    M303, // Coaxial cross-linker
}

/// G-code generator
pub struct GCodeGenerator {
    commands: Vec<String>,
}

impl GCodeGenerator {
    pub fn new() -> Self {
        Self {
            commands: vec![
                "; TPT BioCAD G-code".to_string(),
                "; Licensed under Apache 2.0".to_string(),
            ],
        }
    }

    pub fn add(&mut self, cmd: GCodeCommand) {
        let line = match cmd {
            GCodeCommand::G0 { x, y, z, e } => {
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
                if let Some(v) = e {
                    parts.push(format!("E{:.6}", v));
                }
                parts.join(" ")
            }
            GCodeCommand::G1 { x, y, z, e, f } => {
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
                    parts.push(format!("E{:.6}", v));
                }
                if let Some(v) = f {
                    parts.push(format!("F{:.0}", v));
                }
                parts.join(" ")
            }
            GCodeCommand::G28 => "G28".to_string(),
            GCodeCommand::M104 { s } => format!("M104 S{:.0}", s),
            GCodeCommand::M109 { s } => format!("M109 S{:.0}", s),
            GCodeCommand::M300 { s, p } => format!("M300 S{:.0} P{:.0}", s, p),
            GCodeCommand::M301 { t, r } => format!("M301 T{:.0} R{:.1}", t, r),
            GCodeCommand::M302 { u, t } => format!("M302 U{:.0} T{:.0}", u, t),
            GCodeCommand::M303 => "M303".to_string(),
        };
        self.commands.push(line);
    }

    pub fn generate(&self) -> String {
        self.commands.join("\n")
    }
}

impl Default for GCodeGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl GCodeGenerator {
    /// Convert a sequence of toolpath commands to G-code and append them.
    pub fn add_toolpath(&mut self, commands: &[ToolpathCommand]) {
        for cmd in commands {
            match cmd {
                ToolpathCommand::Move {
                    x,
                    y,
                    z,
                    e,
                    f,
                    rapid,
                } => {
                    let gcmd = if *rapid {
                        GCodeCommand::G0 {
                            x: Some(*x),
                            y: Some(*y),
                            z: Some(*z),
                            e: *e,
                        }
                    } else {
                        GCodeCommand::G1 {
                            x: Some(*x),
                            y: Some(*y),
                            z: Some(*z),
                            e: *e,
                            f: *f,
                        }
                    };
                    self.add(gcmd);
                }
                ToolpathCommand::SetTemperature { temp, wait } => {
                    if *wait {
                        self.add(GCodeCommand::M109 { s: *temp });
                    } else {
                        self.add(GCodeCommand::M104 { s: *temp });
                    }
                }
                ToolpathCommand::PneumaticPressure { pressure, duration } => {
                    self.add(GCodeCommand::M300 {
                        s: *pressure,
                        p: *duration,
                    });
                }
                ToolpathCommand::UVCuring {
                    intensity,
                    duration,
                } => {
                    self.add(GCodeCommand::M302 {
                        u: *intensity,
                        t: *duration,
                    });
                }
                ToolpathCommand::CoaxialFlow => {
                    self.add(GCodeCommand::M303);
                }
            }
        }
    }
}
