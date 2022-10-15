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
    pub fn percent(&mut self, pct: u8) {
        let duty_shift = (self.upper_bound - self.lower_bound) as u32 * (pct as u32) / 100;
        self.pwm.set_duty(self.lower_bound + duty_shift as u16);
    }

    pub fn fraction(&mut self, frac: f32) {
        let duty_shift = (self.upper_bound - self.lower_bound) as f32 * frac;
        self.pwm.set_duty(self.lower_bound + (duty_shift as u16));
    }
}
