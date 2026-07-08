// Async serial connection to a 3D printer
// Licensed under Apache 2.0

use crate::HalError;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::time::timeout;
use tokio_serial::SerialPortBuilderExt;

/// Async connection to a printer via serial port.
pub struct PrinterConnection {
    writer: Box<dyn tokio::io::AsyncWrite + Unpin>,
    reader: BufReader<Box<dyn tokio::io::AsyncRead + Unpin>>,
    connected: bool,
}

impl PrinterConnection {
    /// Connect to a serial port at the given baud rate.
    pub async fn connect(port: &str, baud_rate: u32) -> Result<Self, HalError> {
        let serial = tokio_serial::new(port, baud_rate).open_native_async()?;

        let (reader_half, writer_half) = tokio::io::split(serial);

        Ok(Self {
            writer: Box::new(writer_half),
            reader: BufReader::new(Box::new(reader_half)),
            connected: true,
        })
    }

    /// Send a single G-code line and read until `ok` or error.
    /// Returns the full response text.
    pub async fn send_gcode(&mut self, cmd: &str) -> Result<String, HalError> {
        if !self.connected {
            return Err(HalError::NotConnected);
        }

        // Send command with newline
        self.writer
            .write_all(format!("{}\n", cmd).as_bytes())
            .await?;
        self.writer.flush().await?;

        // Read lines until we get "ok" or "!!" (error)
        let mut response = String::new();
        let read_result = timeout(Duration::from_secs(30), async {
            let mut line = String::new();
            loop {
                line.clear();
                self.reader.read_line(&mut line).await?;
                response.push_str(&line);

                let trimmed = line.trim().to_lowercase();
                if trimmed == "ok" || trimmed.starts_with("!!") || trimmed.starts_with("recv") {
                    break;
                }
            }
            Ok::<_, std::io::Error>(())
        })
        .await;

        match read_result {
            Ok(Ok(())) => {
                if response.trim().to_lowercase().starts_with("!!") {
                    Err(HalError::PrinterError(response.trim().to_string()))
                } else {
                    Ok(response)
                }
            }
            Ok(Err(e)) => Err(HalError::Io(e)),
            Err(_) => Err(HalError::Timeout),
        }
    }

    /// Send multiple G-code lines (one per line in the string).
    pub async fn send_gcode_lines(&mut self, gcode: &str) -> Result<(), HalError> {
        for line in gcode.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }
            self.send_gcode(trimmed).await?;
        }
        Ok(())
    }

    /// Wait for the printer to reach a target temperature.
    /// Polls M105 every 2 seconds until the reported temp is within 1°C of target.
    pub async fn wait_for_temperature(
        &mut self,
        target: f64,
        timeout_dur: Duration,
    ) -> Result<(), HalError> {
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > timeout_dur {
                return Err(HalError::Timeout);
            }

            let response = self.send_gcode("M105").await?;
            // Parse "ok T:25.0 /60.0" format
            if let Some(temp) = parse_temperature(&response) {
                if (temp - target).abs() < 1.0 {
                    return Ok(());
                }
            }

            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    /// Check if the connection is alive.
    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// Disconnect from the printer.
    pub async fn disconnect(&mut self) {
        self.connected = false;
        // Best-effort close — drop the writer/reader on scope exit
    }
}

/// Parse temperature from M105 response like "ok T:25.0 /60.0 B:60.0 /60.0".
fn parse_temperature(response: &str) -> Option<f64> {
    for part in response.split_whitespace() {
        if let Some(rest) = part.strip_prefix("T:") {
            if let Ok(temp) = rest.split('/').next()?.parse::<f64>() {
                return Some(temp);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_temperature() {
        assert_eq!(
            parse_temperature("ok T:25.0 /60.0 B:60.0 /60.0"),
            Some(25.0)
        );
        assert_eq!(parse_temperature("ok T:210.5 /210.0"), Some(210.5));
        assert_eq!(parse_temperature("ok B:60.0 /60.0"), None);
        assert_eq!(parse_temperature("ok"), None);
    }
}
