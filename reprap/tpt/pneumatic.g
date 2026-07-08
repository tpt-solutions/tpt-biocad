; TPT BioCAD — Pneumatic Pressure Control (M300)
; Called as: M98 P"0:/macros/tpt/pneumatic.g" S<pressure_kpa> P<duration_ms>
;
; S = target pressure in kPa (0 = release)
; P = duration in ms (0 = hold until next command)
;
; Requires: global tpt.pneumatic_pin, global tpt.pneumatic_max_kpa
; Licensed under Apache 2.0

if !exists(global.tpt.pneumatic_pin)
  abort "TPT: pneumatic_pin not configured"

var pressure = param.S
var duration = param.P
var max_kpa = global.tpt.pneumatic_max_kpa
var duty = limit(round((pressure / max_kpa) * 255), 0, 255)

; Set PWM duty cycle on the pneumatic pin
M42 P{global.tpt.pneumatic_pin} S{var.duty}

if var.duration > 0
  ; Schedule automatic release
  M42 P{global.tpt.pneumatic_pin} S0 P{var.duration}

echo "TPT: Pneumatic pressure =", var.pressure, "kPa (duty", var.duty, ")"
