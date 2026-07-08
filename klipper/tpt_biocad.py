# Klipper plugin for TPT BioCAD extended G-code commands
# Maps M300-M303 onto Klipper primitives (PWM outputs, heater control, output pins)
#
# Install: copy this file to ~/klipper/klippy/extras/tpt_biocad.py
# Config:  add [tpt_biocad] section to printer.cfg (see README.md)
#
# Licensed under Apache 2.0

import logging

PIN_STATE_DELAY = 0.05  # seconds between pin state changes


class TPTBioCAD:
    def __init__(self, config):
        self.printer = config.get_printer()
        self.reactor = self.printer.get_reactor()
        self.gcode = self.printer.lookup_object('gcode')

        # Register extended G-code commands
        self.gcode.register_command('M300', self.cmd_M300)
        self.gcode.register_command('M301', self.cmd_M301)
        self.gcode.register_command('M302', self.cmd_M302)
        self.gcode.register_command('M303', self.cmd_M303)

        # Load output pin configurations
        self.pneumatic_pin = None
        self.uv_pin = None
        self.coaxial_pin = None
        self.heater = None

        self._load_config(config)
        self.printer.register_event_handler("ready", self._handle_ready)

    def _load_config(self, config):
        """Load pin and heater configurations from printer.cfg."""
        # Pneumatic pressure control pin (PWM-capable)
        pneumatic_pin_name = config.get('pneumatic_pin', None)
        if pneumatic_pin_name:
            self.pneumatic_pin_name = pneumatic_pin_name
            self.pneumatic_max_kpa = config.getfloat('pneumatic_max_kpa', 500.0)
            self.pneumatic_pwm_range = config.getint('pneumatic_pwm_range', 1000)

        # UV curing pin (PWM-capable)
        uv_pin_name = config.get('uv_pin', None)
        if uv_pin_name:
            self.uv_pin_name = uv_pin_name
            self.uv_max_intensity = config.getfloat('uv_max_intensity', 100.0)
            self.uv_pwm_range = config.getint('uv_pwm_range', 255)

        # Coaxial cross-linker pin (digital output)
        coaxial_pin_name = config.get('coaxial_pin', None)
        if coaxial_pin_name:
            self.coaxial_pin_name = coaxial_pin_name

        # Heater name for thermal profiling
        self.heater_name = config.get('heater', None)

    def _handle_ready(self):
        """Look up pin and heater objects after the printer is fully loaded."""
        # Look up pneumatic PWM output
        if hasattr(self, 'pneumatic_pin_name'):
            try:
                self.pneumatic_pin = self.printer.lookup_object(
                    'output_pin %s' % self.pneumatic_pin_name
                )
            except Exception:
                logging.warning(
                    'tpt_biocad: Could not find output_pin %s',
                    self.pneumatic_pin_name
                )

        # Look up UV PWM output
        if hasattr(self, 'uv_pin_name'):
            try:
                self.uv_pin = self.printer.lookup_object(
                    'output_pin %s' % self.uv_pin_name
                )
            except Exception:
                logging.warning(
                    'tpt_biocad: Could not find output_pin %s',
                    self.uv_pin_name
                )

        # Look up coaxial digital output
        if hasattr(self, 'coaxial_pin_name'):
            try:
                self.coaxial_pin = self.printer.lookup_object(
                    'output_pin %s' % self.coaxial_pin_name
                )
            except Exception:
                logging.warning(
                    'tpt_biocad: Could not find output_pin %s',
                    self.coaxial_pin_name
                )

        # Look up heater for thermal profiling
        if self.heater_name:
            try:
                self.heater = self.printer.lookup_object(
                    'heater %s' % self.heater_name
                )
            except Exception:
                logging.warning(
                    'tpt_biocad: Could not find heater %s',
                    self.heater_name
                )
        else:
            # Try to find the first available heater
            try:
                self.heater = self.printer.lookup_object('heater heater_bed')
            except Exception:
                try:
                    self.heater = self.printer.lookup_object('extruder')
                except Exception:
                    logging.warning('tpt_biocad: No heater found for thermal profiling')

    def _set_pwm(self, pin_obj, value, max_value, pwm_range):
        """Set a PWM output pin to a proportional value."""
        if pin_obj is None:
            return
        duty = int((value / max_value) * pwm_range) if max_value > 0 else 0
        duty = max(0, min(pwm_range, duty))
        pin_obj.set_duty_cycle(duty)

    def cmd_M300(self, gcmd):
        """Pneumatic pressure control: M300 S<pressure_kpa> P<duration_ms>

        S = target pressure in kPa (0 = release)
        P = duration in milliseconds (0 = hold until next M300)
        """
        pressure = gcmd.get_float('S', 0.0, minval=0.0)
        duration_ms = gcmd.get_float('P', 0.0, minval=0.0)

        if self.pneumatic_pin is None:
            gcmd.respond_info('tpt_biocad: No pneumatic pin configured, M300 ignored')
            return

        # Set pressure via PWM
        self._set_pwm(
            self.pneumatic_pin, pressure,
            self.pneumatic_max_kpa, self.pneumatic_pwm_range
        )

        gcmd.respond_info(
            'tpt_biocad: Pneumatic pressure set to %.1f kPa' % pressure
        )

        # Schedule automatic release if duration > 0
        if duration_ms > 0:
            def release_pressure(eventtime):
                self._set_pwm(
                    self.pneumatic_pin, 0.0,
                    self.pneumatic_max_kpa, self.pneumatic_pwm_range
                )
                return self.reactor.NEVER

            self.reactor.register_timer(
                release_pressure,
                self.reactor.monotonic() + (duration_ms / 1000.0)
            )

    def cmd_M301(self, gcmd):
        """Thermal profiling: M301 T<temp> R<ramp_rate>

        T = target temperature in degrees C
        R = ramp rate in degrees C/s (0 = use heater's built-in ramp)
        """
        temp = gcmd.get_float('T', 0.0, minval=0.0)
        ramp_rate = gcmd.get_float('R', 0.0, minval=0.0)

        if self.heater is None:
            gcmd.respond_info('tpt_biocad: No heater found, M301 ignored')
            return

        # Use Klipper's heater set_temp for temperature control
        # The ramp rate is advisory -- Klipper's PID controller handles the ramp
        try:
            if hasattr(self.heater, 'set_temp'):
                self.heater.set_temp(temp)
                gcmd.respond_info(
                    'tpt_biocad: Heater set to %.1f C (ramp: %.2f C/s)' % (
                        temp, ramp_rate
                    )
                )
            else:
                # Fallback: use G-code to set temperature
                self.gcode.run_script_from_command('M104 S%.1f' % temp)
                gcmd.respond_info(
                    'tpt_biocad: M104 sent for %.1f C' % temp
                )
        except Exception as e:
            gcmd.respond_info('tpt_biocad: Error setting temperature: %s' % str(e))

    def cmd_M302(self, gcmd):
        """UV curing control: M302 U<intensity> T<duration>

        U = UV intensity as fraction of max (0-100) or W/m2
        T = exposure time in seconds (0 = hold until next M302)
        """
        intensity = gcmd.get_float('U', 0.0, minval=0.0)
        duration_s = gcmd.get_float('T', 0.0, minval=0.0)

        if self.uv_pin is None:
            gcmd.respond_info('tpt_biocad: No UV pin configured, M302 ignored')
            return

        # Set UV intensity via PWM
        self._set_pwm(
            self.uv_pin, intensity,
            self.uv_max_intensity, self.uv_pwm_range
        )

        gcmd.respond_info(
            'tpt_biocad: UV intensity set to %.1f W/m2' % intensity
        )

        # Schedule automatic turn-off if duration > 0
        if duration_s > 0:
            def turn_off_uv(eventtime):
                self._set_pwm(
                    self.uv_pin, 0.0,
                    self.uv_max_intensity, self.uv_pwm_range
                )
                return self.reactor.NEVER

            self.reactor.register_timer(
                turn_off_uv,
                self.reactor.monotonic() + duration_s
            )

    def cmd_M303(self, gcmd):
        """Coaxial cross-linker flow control: M303

        Toggles the coaxial output pin. When called, turns on the
        cross-linker flow. When called again, turns it off.
        Use M303 S0 to explicitly turn off, M303 S1 to turn on.
        """
        state = gcmd.get_int('S', -1, minval=0, maxval=1)

        if self.coaxial_pin is None:
            gcmd.respond_info('tpt_biocad: No coaxial pin configured, M303 ignored')
            return

        # Toggle or set explicit state
        if state == -1:
            # Toggle: read current state and flip
            current = getattr(self.coaxial_pin, 'last_value', 0)
            new_state = 0 if current else 1
        else:
            new_state = state

        if hasattr(self.coaxial_pin, 'set_duty_cycle'):
            self.coaxial_pin.set_duty_cycle(new_state)
        elif hasattr(self.coaxial_pin, 'set_pin'):
            self.coaxial_pin.set_pin(new_state)

        state_str = "ON" if new_state else "OFF"
        gcmd.respond_info(
            'tpt_biocad: Coaxial cross-linker flow %s' % state_str
        )


def load_config(config):
    return TPTBioCAD(config)
