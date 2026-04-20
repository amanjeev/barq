//! ESP32-C3 blinky example — produces an ELF with realistic Flash/RAM usage
//! for testing barq visualization.

#![no_std]
#![no_main]

#[allow(unused_imports)]
use esp_backtrace as _;
use esp_hal::delay::Delay;
use esp_hal::gpio::{Level, Output, OutputConfig};
use esp_hal::main;
use esp_println::println;

// Static variables — will appear in DRAM (.bss)
static mut SAMPLE_BUFFER: [u32; 128] = [0u32; 128];
static mut TICK_COUNT: u64 = 0;

// Initialized data — will appear in .data (DRAM, initialized from Flash)
static FILTER_COEFFICIENTS: [f32; 16] = [
    0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625, 0.0625,
    0.0625, 0.0625, 0.0625, 0.0625, 0.0625,
];

static DEVICE_ID: &str = "barq-test-esp32c3";

#[main]
fn main() -> ! {
    let config = esp_hal::Config::default();
    let peripherals = esp_hal::init(config);

    // GPIO setup — LED on GPIO8 (common on ESP32-C3 dev boards)
    let mut led = Output::new(peripherals.GPIO8, Level::Low, OutputConfig::default());

    let delay = Delay::new();

    println!("barq ESP32-C3 test firmware booted");
    println!("Device: {}", DEVICE_ID);

    let mut iteration: u32 = 0;

    loop {
        led.set_high();
        delay.delay_millis(500);

        led.set_low();
        delay.delay_millis(500);

        iteration = iteration.wrapping_add(1);
        update_samples(iteration);
        let avg = compute_moving_average();

        if iteration.is_multiple_of(10) {
            println!("Tick {}: avg={}", iteration, avg);
        }
    }
}

/// Fill sample buffer with synthetic data — exercises DRAM
fn update_samples(tick: u32) {
    unsafe {
        TICK_COUNT = TICK_COUNT.wrapping_add(1);
        for (i, slot) in SAMPLE_BUFFER.iter_mut().enumerate() {
            *slot = tick.wrapping_add(i as u32).wrapping_mul(17);
        }
    }
}

/// FIR filter — adds meaningful .text and uses FILTER_COEFFICIENTS from .rodata
fn compute_moving_average() -> f32 {
    let mut sum: f32 = 0.0;
    unsafe {
        for (i, &coeff) in FILTER_COEFFICIENTS.iter().enumerate() {
            sum += SAMPLE_BUFFER[i] as f32 * coeff;
        }
    }
    sum
}

/// CRC-like checksum — more .text content
fn simple_checksum(data: &[u32]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &word in data {
        crc ^= word;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// Prevent optimization from removing our functions
#[used]
static KEEP: [fn(&[u32]) -> u32; 1] = [simple_checksum];
