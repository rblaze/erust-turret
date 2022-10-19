#![deny(unsafe_code)]

use embedded_hal::PwmPin;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bounds<PWM: PwmPin> {
    pub lower_bound: PWM::Duty,
    pub upper_bound: PWM::Duty,
}

impl<PWM> Bounds<PWM>
where
    PWM: PwmPin<Duty = u16>,
{
    // Automatically calculate 1ms/2ms bounds used by many servos.
    pub fn from_period_ms(pwm: &PWM, period_ms: u16) -> Self {
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

        Self {
            lower_bound,
            upper_bound,
        }
    }
}

pub struct Servo<PWM: PwmPin> {
    pwm: PWM,
    bounds: Bounds<PWM>,
}

impl<PWM: PwmPin> Servo<PWM> {
    pub fn new(pwm: PWM, bounds: Bounds<PWM>) -> Self {
        Servo { pwm, bounds }
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
        let duty_shift =
            (self.bounds.upper_bound - self.bounds.lower_bound) as u32 * (pct as u32) / 100;
        self.pwm
            .set_duty(self.bounds.lower_bound + duty_shift as u16);
    }

    pub fn fraction(&mut self, frac: f32) {
        let duty_shift = (self.bounds.upper_bound - self.bounds.lower_bound) as f32 * frac;
        self.pwm
            .set_duty(self.bounds.lower_bound + (duty_shift as u16));
    }
}
