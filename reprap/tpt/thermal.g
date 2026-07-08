; TPT BioCAD — Thermal Profiling (M301)
; Called as: M98 P"0:/macros/tpt/thermal.g" S<temp_c> R<ramp_rate>
;
; S = target temperature in °C
; R = ramp rate in °C/s (advisory for RepRap PID)
;
; Licensed under Apache 2.0

var temp = param.S
var ramp = param.R

if var.temp > 0
  ; Set heater target (uses H0 by default; customize heater number as needed)
  M140 H0 S{var.temp}
  echo "TPT: Heater target =", var.temp, "°C, ramp rate =", var.ramp, "°C/s"
else
  echo "TPT: Invalid temperature"
