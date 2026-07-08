# TPT BioCAD — Klipper Plugin

Klipper plugin implementing M300-M303 extended G-code commands for bioprinting and food printing.

## Installation

1. Copy `tpt_biocad.py` to your Klipper extras directory:
   ```bash
   cp tpt_biocad.py ~/klipper/klippy/extras/
   ```

2. Restart Klipper:
   ```bash
   sudo systemctl restart klipper
   ```

## Configuration

Add a `[tpt_biocad]` section to your `printer.cfg`:

```ini
[tpt_biocad]
# Pneumatic pressure control (M300)
pneumatic_pin: gpio20              # PWM-capable pin for pressure regulator
pneumatic_max_kpa: 500.0           # Max pressure the regulator can deliver
pneumatic_pwm_range: 1000          # PWM range for the regulator

# UV curing (M302)
uv_pin: gpio21                     # PWM-capable pin for UV LED array
uv_max_intensity: 100.0            # Max UV intensity in W/m^2
uv_pwm_range: 255                  # PWM range for UV LEDs

# Coaxial cross-linker (M303)
coaxial_pin: gpio22                # Digital output for coaxial valve

# Thermal profiling (M301) — optional, defaults to heater_bed
heater: heater_bed                 # Which heater to control
```

All pin entries are optional. Commands for unconfigured pins are silently ignored with a log warning.

## G-code Commands

### M300 — Pneumatic Pressure Control

```
M300 S<pressure_kpa> P<duration_ms>
```

- `S`: Target pressure in kPa (0 = release pressure)
- `P`: Duration in milliseconds (0 = hold until next M300)

Example: `M300 S100 P500` — apply 100 kPa for 500 ms, then release.

### M301 — Thermal Profiling

```
M301 T<temp> R<ramp_rate>
```

- `T`: Target temperature in °C
- `R`: Ramp rate in °C/s (advisory; Klipper's PID handles the actual ramp)

Example: `M301 T31.0 R0.50` — heat to 31°C for chocolate tempering.

### M302 — UV Curing

```
M302 U<intensity> T<duration>
```

- `U`: UV intensity (W/m² or fraction of max, depending on hardware)
- `T`: Exposure time in seconds (0 = hold until next M302)

Example: `M302 U10 T30` — expose at 10 W/m² for 30 seconds.

### M303 — Coaxial Cross-linker Flow

```
M303
M303 S<state>
```

- No argument: toggle cross-linker flow on/off
- `S1`: Turn on
- `S0`: Turn off

## How It Works

The plugin maps extended commands onto Klipper's existing primitives:

| Command | Klipper Primitive |
|---------|-------------------|
| M300 (pressure) | `output_pin` with PWM duty cycle |
| M301 (temperature) | `heater.set_temp()` or fallback to M104 |
| M302 (UV) | `output_pin` with PWM duty cycle |
| M303 (coaxial) | `output_pin` digital toggle |

## License

Apache License 2.0
