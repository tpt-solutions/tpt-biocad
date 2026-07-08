// TPT HAL - Hardware Abstraction Layer
// Licensed under Apache 2.0
//
// Async serial communication with 3D printers for bioprinting/food printing.
// Supports pneumatic (M300 pressure control) and screw (E-axis stepper)
// extruders. Klipper is the primary firmware target.

pub mod commands;
pub mod feedback;
pub mod firmware;
pub mod monitoring;
pub mod serial;

pub use commands::*;
pub use feedback::*;
pub use firmware::*;
pub use monitoring::*;
pub use serial::*;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum HalError {
    #[error("serial port error: {0}")]
    Serial(#[from] tokio_serial::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("printer not connected")]
    NotConnected,
    #[error("printer error: {0}")]
    PrinterError(String),
    #[error("timeout waiting for response")]
    Timeout,
}
