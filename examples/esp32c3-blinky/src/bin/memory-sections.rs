//! memory-sections workload. Shared source makes the two architecture fixtures comparable.
#![no_std]
#![no_main]

use esp_hal::main;
use panic_halt as _;

#[path = "../../../workloads/memory_sections.rs"]
mod workload;

#[main]
fn main() -> ! {
    esp_hal::init(esp_hal::Config::default());
    let mut tick = 0u32;
    loop {
        core::hint::black_box(workload::step(tick));
        tick = tick.wrapping_add(1);
    }
}
