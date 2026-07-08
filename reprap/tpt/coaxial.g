; TPT BioCAD — Coaxial Cross-linker Control (M303)
; Called as: M98 P"0:/macros/tpt/coaxial.g" S<state>
;
; S = -1: toggle on/off
; S = 0: turn off
; S = 1: turn on
;
; Requires: global tpt.coaxial_pin
; Licensed under Apache 2.0

if !exists(global.tpt.coaxial_pin)
  abort "TPT: coaxial_pin not configured"

var state = param.S

if var.state == -1
  ; Toggle: read current state and flip
  if exists(global.tpt.coaxial_state)
    set global.tpt.coaxial_state = !global.tpt.coaxial_state
  else
    global tpt.coaxial_state = true
  var.new_state = global.tpt.coaxial_state
elif var.state == 1
  global tpt.coaxial_state = true
  var.new_state = true
else
  global tpt.coaxial_state = false
  var.new_state = false

if var.new_state
  M42 P{global.tpt.coaxial_pin} S1
  echo "TPT: Coaxial cross-linker ON"
else
  M42 P{global.tpt.coaxial_pin} S0
  echo "TPT: Coaxial cross-linker OFF"
