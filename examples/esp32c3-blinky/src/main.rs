//! ESP32-C3 blinky example — produces an ELF with realistic Flash/RAM usage
//! for testing barq visualization.

#![no_std]
#![no_main]

use core::cell::{Cell, RefCell};
use critical_section::Mutex;

use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::main;
#[allow(unused_imports)]
use panic_halt as _;

// Same workload as the STM32 example: counter, sensor buffer, and calibration.
static BLINK_COUNT: Mutex<Cell<u32>> = Mutex::new(Cell::new(0));
static SENSOR_BUFFER: Mutex<RefCell<[u16; 64]>> = Mutex::new(RefCell::new([0; 64]));

// Read-only calibration data — normally placed in .rodata.
static CALIBRATION_TABLE: [f32; 8] = [1.0, 1.02, 0.98, 1.01, 0.99, 1.03, 0.97, 1.0];

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    // GPIO setup — LED on GPIO8 (common on ESP32-C3 dev boards)
    let mut led = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());

    let delay = Delay::new();

    loop {
        led.set_high();
        delay.delay_millis(500);

        led.set_low();
        delay.delay_millis(500);

        increment_blink_count();
        update_sensor_buffer();
    }
}

fn increment_blink_count() {
    critical_section::with(|cs| {
        let count = BLINK_COUNT.borrow(cs);
        count.set(count.get().wrapping_add(1));
    });
}

fn update_sensor_buffer() {
    // Simulate filling a sensor buffer — exercises .bss memory
    critical_section::with(|cs| {
        let mut buffer = SENSOR_BUFFER.borrow(cs).borrow_mut();
        for (i, slot) in buffer.iter_mut().enumerate() {
            *slot = (i as u16).wrapping_mul(CALIBRATION_TABLE[i % 8] as u16);
        }
    });
}

/// A helper that does some math — adds more .text content
fn apply_calibration(raw: u16, channel: usize) -> f32 {
    let factor = CALIBRATION_TABLE[channel % CALIBRATION_TABLE.len()];
    raw as f32 * factor
}

// Make sure these functions aren't optimized away
#[used]
static KEEP_FNS: [fn(u16, usize) -> f32; 1] = [apply_calibration];
