# TPT BioCAD — RepRapFirmware Plugin

M300-M303 extended G-code commands for bioprinting/food printing on RepRapFirmware (Duet boards).

## Installation

1. Copy the `tpt/` macro directory to your Duet's SD card:
   ```bash
   scp -r tpt/ duet:/sys/macros/tpt/
   ```

2. Add pin aliases to `config.g`:
   ```gcode
   ; TPT BioCAD pin configuration
   global tpt.pneumatic_pin = gpio20
   global tpt.pneumatic_max_kpa = 500.0
   global tpt.uv_pin = gpio21
   global tpt.uv_max_intensity = 100.0
   global tpt.coaxial_pin = gpio22
   ```

3. Restart firmware or run `M999`.

## Macro Files

### tpt/pneumatic.g — M300 handler
Called as: `M98 P"0:/macros/tpt/pneumatic.g" S<pressure> P<duration>`

Controls a pneumatic pressure regulator via PWM output.

### tpt/thermal.g — M301 handler  
Called as: `M98 P"0:/macros/tpt/thermal.g" S<temp> R<ramp_rate>`

Sets heater target temperature. Ramp rate is advisory; PID handles the actual
ramp.

### tpt/uv.g — M302 handler
Called as: `M98 P"0:/macros/tpt/uv.g" S<intensity> T<duration>`

Controls UV LED array via PWM output. Auto-off after duration if T>0.

### tpt/coaxial.g — M303 handler
Called as: `M98 P"0:/macros/tpt/coaxial.g" S<state>`
- S = -1: toggle
- S = 0: off  
- S = 1: on

Controls coaxial cross-linker valve (digital on/off).

## License

Apache License 2.0
