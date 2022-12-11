#![cfg_attr(not(test), no_std)]
#![deny(unsafe_code)]

use core::fmt::Debug;
use core::fmt::Display;

use embedded_hal::PwmPin;
use num::rational::Ratio;
use num::CheckedAdd;
use num::One;
use num::Zero;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Error {
    ZeroPeriod,
    UpperBoundOverflow,
    InvalidArgument,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Bounds<PWM: PwmPin> {
    pub lower_bound: PWM::Duty,
    pub width: PWM::Duty,
}

// Manual implementation to avoid requiring Debug for PWM
impl<PWM> Debug for Bounds<PWM>
where
    PWM: PwmPin,
    PWM::Duty: Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bounds")
            .field("lower_bound", &self.lower_bound)
            .field("width", &self.width)
            .finish()
    }
}

impl<PWM> Display for Bounds<PWM>
where
    PWM: PwmPin,
    PWM::Duty: Display,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Bounds({}, {})", self.lower_bound, self.width)
    }
}

impl<PWM> Bounds<PWM>
where
    PWM: PwmPin<Duty = u16>,
{
    // Calculate 1ms/2ms bounds used by many servos.
    fn get_full_bounds(pwm: &PWM, period_ms: PWM::Duty) -> Result<(Ratio<u32>, Ratio<u32>), Error> {
        if period_ms.is_zero() {
            return Err(Error::ZeroPeriod);
        }

        // Use u32 to avoid overflow
        let max_duty = Ratio::from_integer(pwm.get_max_duty() as u32);
        let lower_bound = max_duty / period_ms as u32; // 1ms
        let width = lower_bound; // also 1 ms

        let upper_bound = lower_bound.checked_add(&width);
        if upper_bound.is_none() || upper_bound.unwrap() > max_duty {
            return Err(Error::UpperBoundOverflow);
        }

        Ok((lower_bound, width))
    }

    pub fn from_period_ms(pwm: &PWM, period_ms: PWM::Duty) -> Result<Self, Error> {
        let (lower_bound, width) = Self::get_full_bounds(pwm, period_ms)?;

        Ok(Self {
            lower_bound: lower_bound.to_integer() as PWM::Duty,
            width: width.to_integer() as PWM::Duty,
        })
    }

    // Calculate 1/2ms bounds shrunk by a scale factor.
    pub fn scale_from_period_ms(
        pwm: &PWM,
        period_ms: PWM::Duty,
        scale: Ratio<PWM::Duty>,
    ) -> Result<Self, Error> {
        let (lower_base, full_width) = Self::get_full_bounds(pwm, period_ms)?;

        if scale > Ratio::one() {
            return Err(Error::InvalidArgument);
        }

        // Use u32 to avoid overflow
        let scale32 = Ratio::new(*scale.numer() as u32, *scale.denom() as u32);

        let midpoint = lower_base + full_width / 2;
        let width = full_width * scale32;
        let lower_bound = midpoint - (width / 2);

        Ok(Self {
            lower_bound: lower_bound.to_integer() as PWM::Duty,
            width: width.to_integer() as PWM::Duty,
        })
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
    fn calculate_duty(&self, ratio: Ratio<PWM::Duty>) -> Result<PWM::Duty, Error> {
        if ratio > Ratio::one() {
            return Err(Error::InvalidArgument);
        }

        // Use u32 to avoid overflow
        let ratio32 = Ratio::new(*ratio.numer() as u32, *ratio.denom() as u32);
        let shift = Ratio::from_integer(self.bounds.width as u32) * ratio32;

        Ok(self.bounds.lower_bound + shift.to_integer() as PWM::Duty)
    }

    pub fn set(&mut self, ratio: Ratio<PWM::Duty>) -> Result<(), Error> {
        self.pwm.set_duty(self.calculate_duty(ratio)?);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use embedded_hal::PwmPin;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]

    struct TestPwmPin {
        duty: u16,
        max_duty: u16,
    }

    impl PwmPin for TestPwmPin {
        type Duty = u16;

        fn disable(&mut self) {}
        fn enable(&mut self) {}
        fn get_duty(&self) -> Self::Duty {
            self.duty
        }
        fn get_max_duty(&self) -> Self::Duty {
            self.max_duty
        }
        fn set_duty(&mut self, duty: Self::Duty) {
            self.duty = duty
        }
    }

    #[test]
    fn test_zero_period() {
        let pin = TestPwmPin {
            duty: 0,
            max_duty: 53333,
        };

        let bounds = Bounds::from_period_ms(&pin, 0);
        assert_eq!(bounds, Err(Error::ZeroPeriod));
    }

    #[test]
    fn test_big_scale() {
        let pin = TestPwmPin {
            duty: 0,
            max_duty: 53333,
        };

        let bounds = Bounds::scale_from_period_ms(&pin, 50, Ratio::new(101, 100));
        assert_eq!(bounds, Err(Error::InvalidArgument));
    }

    #[test]
    fn test_bounds() {
        let pin = TestPwmPin {
            duty: 0,
            max_duty: 53333,
        };

        let bounds = Bounds::from_period_ms(&pin, 50);
        assert_eq!(
            bounds,
            Ok(Bounds {
                lower_bound: 1066,
                width: 1066
            })
        );
    }

    #[test]
    fn test_scaled_one_to_one_bounds() {
        let pin = TestPwmPin {
            duty: 0,
            max_duty: 53333,
        };

        let bounds = Bounds::scale_from_period_ms(&pin, 50, Ratio::from_integer(1));
        assert_eq!(
            bounds,
            Ok(Bounds {
                lower_bound: 1066,
                width: 1066
            })
        );
    }

    #[test]
    fn test_scaled_bounds() {
        let pin = TestPwmPin {
            duty: 0,
            max_duty: 53333,
        };

        let bounds = Bounds::scale_from_period_ms(&pin, 50, Ratio::new(40000, 65535));
        assert_eq!(
            bounds,
            Ok(Bounds {
                lower_bound: 1274,
                width: 651
            })
        );
    }

    #[test]
    fn test_full_range() {
        let pin = TestPwmPin {
            duty: 0,
            max_duty: 53333,
        };

        let bounds = Bounds::from_period_ms(&pin, 50).unwrap();
        let servo = Servo::new(pin, bounds);

        for i in 0..=100 {
            let ratio = Ratio::new(i, 100);
            let duty = servo.calculate_duty(ratio);
            assert!(duty.is_ok());
            assert_eq!(duty.unwrap() as u32, 1066 + (1066 * i as u32) / 100);
        }
    }
}
