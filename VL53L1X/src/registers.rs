// Constants from STM driver.

pub type Register = u16;

pub const VL53L1_VHV_CONFIG__TIMEOUT_MACROP_LOOP_BOUND: Register = 0x08;
pub const GPIO_HV_MUX__CTRL: Register = 0x30;
pub const GPIO__TIO_HV_STATUS: Register = 0x31;
pub const PHASECAL_CONFIG__TIMEOUT_MACROP: Register = 0x4b;
pub const RANGE_CONFIG__TIMEOUT_MACROP_A_HI: Register = 0x5e;
pub const RANGE_CONFIG__VCSEL_PERIOD_A: Register = 0x60;
pub const RANGE_CONFIG__TIMEOUT_MACROP_B_HI: Register = 0x61;
pub const RANGE_CONFIG__VCSEL_PERIOD_B: Register = 0x63;
pub const RANGE_CONFIG__VALID_PHASE_HIGH: Register = 0x69;
pub const VL53L1_SYSTEM__INTERMEASUREMENT_PERIOD: Register = 0x6c;
pub const SD_CONFIG__WOI_SD0: Register = 0x78;
pub const SD_CONFIG__INITIAL_PHASE_SD0: Register = 0x7a;
pub const SYSTEM__INTERRUPT_CLEAR: Register = 0x86;
pub const SYSTEM__MODE_START: Register = 0x87;
pub const VL53L1_RESULT__RANGE_STATUS: Register = 0x89;
pub const VL53L1_RESULT__FINAL_CROSSTALK_CORRECTED_RANGE_MM_SD0: Register = 0x96;
pub const VL53L1_RESULT__OSC_CALIBRATE_VAL: Register = 0xde;
pub const VL53L1_FIRMWARE__SYSTEM_STATUS: Register = 0xe5;
