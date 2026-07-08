; TPT BioCAD — UV Curing Control (M302)
; Called as: M98 P"0:/macros/tpt/uv.g" S<intensity> T<duration_s>
;
; S = UV intensity (W/m², 0-100)
; T = exposure time in seconds (0 = hold)
;
; Requires: global tpt.uv_pin, global tpt.uv_max_intensity
; Licensed under Apache 2.0

if !exists(global.tpt.uv_pin)
  abort "TPT: uv_pin not configured"

var intensity = param.S
var duration = param.T
var max_intensity = global.tpt.uv_max_intensity
var duty = limit(round((var.intensity / var.max_intensity) * 255), 0, 255)

; Set UV PWM
M42 P{global.tpt.uv_pin} S{var.duty}

if var.duration > 0
  ; Auto-off after duration seconds
  M42 P{global.tpt.uv_pin} S0 P{var.duration * 1000}

echo "TPT: UV intensity =", var.intensity, "W/m² (duty", var.duty, ")"
