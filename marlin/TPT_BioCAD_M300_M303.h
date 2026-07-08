/**
 * TPT BioCAD — Marlin Extended M-Code Plugin Header
 *
 * Enables M300-M303 for bioprinting/food printing on Marlin firmware.
 *
 * Installation:
 *   1. Copy this file to Marlin/src/feature/tpt_biocad.h
 *   2. Add #include "feature/tpt_biocad.h" to Configuration_adv.h
 *   3. Add the G-code handlers in gcode.cpp (see documentation)
 *
 * Licensed under Apache 2.0
 */

#pragma once

#include "../../core/macros.h"
#include "../../core/serial.h"
#include "../../module/planner.h"
#include "../../module/stepper.h"
#include "../../module/temperature.h"

// Pin definitions — override in Configuration_adv.h
#ifndef TPT_PNEUMATIC_PIN
  #define TPT_PNEUMATIC_PIN -1  // Not configured
#endif

#ifndef TPT_UV_PIN
  #define TPT_UV_PIN -1
#endif

#ifndef TPT_COAXIAL_PIN
  #define TPT_COAXIAL_PIN -1
#endif

#ifndef TPT_PNEUMATIC_MAX_KPA
  #define TPT_PNEUMATIC_MAX_KPA 500.0
#endif

#ifndef TPT_UV_MAX_W_M2
  #define TPT_UV_MAX_W_M2 100.0
#endif

/**
 * M300 - Pneumatic Pressure Control
 *
 * S<pressure_kpa> - Target pressure in kPa (0 = release)
 * P<duration_ms>  - Duration in milliseconds (0 = hold)
 */
void GcodeSuite::M300_Pneumatic() {
  const float pressure = parser.floatval('S', 0.0);
  const int duration = parser.intval('P', 0);

  if (TPT_PNEUMATIC_PIN == -1) {
    SERIAL_ECHOLNPGM("TPT: No pneumatic pin configured");
    return;
  }

  // Convert kPa to PWM duty cycle (0-255)
  const int duty = constrain(
    (int)((pressure / TPT_PNEUMATIC_MAX_KPA) * 255.0),
    0, 255
  );

  WRITE(TPT_PNEUMATIC_PIN, duty);

  if (duration > 0) {
    // Schedule release using the Marlin timer
    planner.synchronize();  // Ensure it's safe to delay
    safe_delay(duration);
    WRITE(TPT_PNEUMATIC_PIN, 0);
  }

  SERIAL_ECHOLNPGM("TPT: Pneumatic pressure set to ", pressure, " kPa");
}

/**
 * M301 - Thermal Profiling
 *
 * T<temp>      - Target temperature in °C
 * R<ramp_rate> - Ramp rate in °C/s (advisory)
 */
void GcodeSuite::M301_Thermal() {
  const float temp = parser.floatval('T', 0.0);

  if (temp > 0) {
    thermalManager.setTargetHotend(temp, 0);
    SERIAL_ECHOLNPGM("TPT: Hotend target set to ", temp, " C");
  }
}

/**
 * M302 - UV Curing Control
 *
 * U<intensity> - UV intensity (W/m² or 0-100%)
 * T<duration>  - Exposure time in seconds (0 = hold)
 */
void GcodeSuite::M302_UV() {
  const float intensity = parser.floatval('U', 0.0);
  const int duration_s = parser.intval('T', 0);

  if (TPT_UV_PIN == -1) {
    SERIAL_ECHOLNPGM("TPT: No UV pin configured");
    return;
  }

  const int duty = constrain(
    (int)((intensity / TPT_UV_MAX_W_M2) * 255.0),
    0, 255
  );

  WRITE(TPT_UV_PIN, duty);

  if (duration_s > 0) {
    planner.synchronize();
    safe_delay(duration_s * 1000);
    WRITE(TPT_UV_PIN, 0);
  }

  SERIAL_ECHOLNPGM("TPT: UV set to ", intensity, " W/m²");
}

/**
 * M303 - Coaxial Cross-linker Control
 *
 * No args    - Toggle
 * S0         - Off
 * S1         - On
 */
void GcodeSuite::M303_Coaxial() {
  if (TPT_COAXIAL_PIN == -1) {
    SERIAL_ECHOLNPGM("TPT: No coaxial pin configured");
    return;
  }

  const int state = parser.intval('S', -1);
  int new_state;

  if (state == -1) {
    // Toggle
    static bool coaxial_state = false;
    coaxial_state = !coaxial_state;
    new_state = coaxial_state ? 1 : 0;
  } else {
    new_state = state;
  }

  WRITE(TPT_COAXIAL_PIN, new_state ? HIGH : LOW);
  SERIAL_ECHOLNPGM("TPT: Coaxial cross-linker ", new_state ? "ON" : "OFF");
}
