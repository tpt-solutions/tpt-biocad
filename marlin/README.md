# TPT BioCAD — Marlin Firmware Plugin

M300-M303 extended G-code commands for bioprinting/food printing on Marlin firmware.

## Installation

1. Copy `tpt_biocad.h` and `tpt_biocad.cpp` to your Marlin `Marlin/src/feature/` directory.

2. Add to `Marlin/src/core/serial.h` or your `Configuration_adv.h`:
   ```cpp
   #include "feature/tpt_biocad.h"
   ```

3. Add the following lines to `Marlin/src/gcode/gcode.cpp` in the process_parsed command handler, or use Marlin's `GcodeSuite`:

   In `Marlin/src/gcode/gcode.h`, add:
   ```cpp
   void M300_Pneumatic();
   void M301_Thermal();
   void M302_UV();
   void M303_Coaxial();
   ```

   In `Marlin/src/gcode/gcode.cpp`, register:
   ```cpp
   case 300: M300_Pneumatic(); break;
   case 301: M301_Thermal(); break;
   case 302: M302_UV(); break;
   case 303: M303_Coaxial(); break;
   ```

4. Configure pins in `Configuration_adv.h`:
   ```cpp
   #define TPT_PNEUMATIC_PIN PC3   // PWM pin for pressure regulator
   #define TPT_UV_PIN PC4          // PWM pin for UV LED
   #define TPT_COAXIAL_PIN PC5     // Digital pin for coaxial valve
   #define TPT_HEATER_PIN HEATER_0 // Heater for thermal profiling
   #define TPT_PNEUMATIC_MAX_KPA 500.0
   #define TPT_UV_MAX_W_M2 100.0
   ```

## G-code Commands

### M300 — Pneumatic Pressure Control
```
M300 S<pressure_kpa> P<duration_ms>
```
S: Target pressure in kPa (0 = release)
P: Duration in ms (0 = hold)

### M301 — Thermal Profiling
```
M301 T<temp> R<ramp_rate>
```
T: Target temperature in °C
R: Ramp rate in °C/s

### M302 — UV Curing
```
M302 U<intensity> T<duration>
```
U: UV intensity (W/m²)
T: Exposure time in seconds

### M303 — Coaxial Cross-linker
```
M303         ; toggle
M303 S0      ; off
M303 S1      ; on
```

## License

Apache License 2.0
