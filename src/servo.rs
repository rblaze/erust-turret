#![deny(unsafe_code)]

use embedded_hal::PwmPin;

pub struct Servo<PWM: PwmPin> {
    pwm: PWM,
    lower_bound: PWM::Duty,
    upper_bound: PWM::Duty,
}

impl<PWM: PwmPin> Servo<PWM> {
    pub fn new(pwm: PWM, lower_bound: PWM::Duty, upper_bound: PWM::Duty) -> Self {
        Servo {
            pwm,
            lower_bound,
            upper_bound,
        }
    }

    pub fn enable(&mut self) {
        self.pwm.enable();
    }

    pub fn disable(&mut self) {
        self.pwm.disable();
    }

    pub fn release(self) -> PWM {
        self.pwm
    }
}

impl<PWM> Servo<PWM>
where
    PWM: PwmPin<Duty = u16>,
{
    // Automatically calculate 1ms/2ms bounds used by many servos.
    pub fn from_period_ms(pwm: PWM, period_ms: u16) -> Self {
        let max_duty = pwm.get_max_duty();
        let lower_bound = max_duty / period_ms;
        let upper_bound_32 = (max_duty as u32 * 2) / (period_ms as u32);

        let upper_bound: PWM::Duty = if upper_bound_32 <= max_duty as u32 {
            upper_bound_32 as PWM::Duty
        } else {
            panic!(
                "Calculated 2ms duty is larger than max_duty: {} > {}",
                upper_bound_32, max_duty
            );
        };

        Self::new(pwm, lower_bound, upper_bound)
    }

    pub fn percent(&mut self, pct: u8) {
        let duty_shift = (self.upper_bound - self.lower_bound) as u32 * (pct as u32) / 100;
        self.pwm.set_duty(self.lower_bound + duty_shift as u16);
    }

    pub fn fraction(&mut self, frac: f32) {
        let duty_shift = (self.upper_bound - self.lower_bound) as f32 * frac;
        self.pwm.set_duty(self.lower_bound + (duty_shift as u16));
    }
}
