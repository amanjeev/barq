//! STM32F411 blinky example — produces an ELF with realistic Flash/RAM usage
//! for testing barq visualization.

#![no_std]
#![no_main]

use core::cell::RefCell;
use cortex_m::interrupt::Mutex;
use cortex_m_rt::entry;
#[allow(unused_imports)]
use panic_halt as _;
use stm32f4xx_hal::{
    pac,
    prelude::*,
};

// Static variables — these will show up in RAM (.bss and .data sections)
static BLINK_COUNT: Mutex<RefCell<u32>> = Mutex::new(RefCell::new(0));
static mut SENSOR_BUFFER: [u16; 64] = [0u16; 64];

// Initialized data — goes in .data (copied from Flash to RAM at boot)
static CALIBRATION_TABLE: [f32; 8] = [1.0, 1.02, 0.98, 1.01, 0.99, 1.03, 0.97, 1.0];

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::Peripherals::take().unwrap();

    // Clock setup — RCC configuration
    let rcc = dp.RCC.constrain();
    let clocks = rcc.cfgr.use_hse(25.MHz()).sysclk(100.MHz()).freeze();

    // GPIO setup — LED on PC13 (common on many STM32F4 boards)
    let gpioc = dp.GPIOC.split();
    let mut led = gpioc.pc13.into_push_pull_output();

    // SysTick delay
    let mut delay = cp.SYST.delay(&clocks);

    // Some ADC setup to add more symbols
    let gpioa = dp.GPIOA.split();
    let _adc_pin = gpioa.pa0.into_analog();

    loop {
        led.set_high();
        delay.delay_ms(500u32);

        led.set_low();
        delay.delay_ms(500u32);

        increment_blink_count();
        update_sensor_buffer();
    }
}

fn increment_blink_count() {
    cortex_m::interrupt::free(|cs| {
        let mut count = BLINK_COUNT.borrow(cs).borrow_mut();
        *count = count.wrapping_add(1);
    });
}

fn update_sensor_buffer() {
    // Simulate filling a sensor buffer — exercises .bss memory
    unsafe {
        for (i, slot) in SENSOR_BUFFER.iter_mut().enumerate() {
            *slot = (i as u16).wrapping_mul(CALIBRATION_TABLE[i % 8] as u16);
        }
    }
}

/// A helper that does some math — adds more .text content
fn apply_calibration(raw: u16, channel: usize) -> f32 {
    let factor = CALIBRATION_TABLE[channel % CALIBRATION_TABLE.len()];
    raw as f32 * factor
}

// Make sure these functions aren't optimized away
#[used]
static KEEP_FNS: [fn(u16, usize) -> f32; 1] = [apply_calibration];
