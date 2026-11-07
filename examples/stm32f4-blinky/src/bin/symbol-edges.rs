//! symbol-edges workload. Shared source makes the two architecture fixtures comparable.
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;
use stm32f4xx_hal as _; // Link the device interrupt vectors.

#[path = "../../../workloads/symbol_edges.rs"]
mod workload;

#[entry]
fn main() -> ! {
    let mut tick = 0u32;
    loop {
        core::hint::black_box(workload::step(tick));
        tick = tick.wrapping_add(1);
    }
}
