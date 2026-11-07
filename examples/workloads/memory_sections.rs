//! Identical section fixtures on both architectures. Sizes exclude runtime data.
use core::hint::black_box;
use core::sync::atomic::{AtomicU32, Ordering};

// 512 bytes of zero-initialized writable data.
#[no_mangle]
pub static BARQ_ZERO_BUFFER: [AtomicU32; 128] = [const { AtomicU32::new(0) }; 128];

// 128 bytes of initialized writable data, copied into RAM at startup.
#[no_mangle]
pub static BARQ_INITIALIZED_BUFFER: [AtomicU32; 32] = [const { AtomicU32::new(7) }; 32];

// 256 bytes of read-only data. black_box at the use site prevents constant folding.
#[no_mangle]
pub static BARQ_LOOKUP_TABLE: [u32; 64] = [0x1234_5678; 64];

pub fn step(tick: u32) -> u32 {
    let table = black_box(&BARQ_LOOKUP_TABLE);
    for (i, slot) in BARQ_ZERO_BUFFER.iter().enumerate() {
        slot.store(tick.wrapping_add(table[i % table.len()]), Ordering::Relaxed);
    }
    for slot in &BARQ_INITIALIZED_BUFFER {
        // This workload is the only writer; no atomic read-modify-write is needed.
        slot.store(
            slot.load(Ordering::Relaxed).wrapping_add(tick),
            Ordering::Relaxed,
        );
    }
    BARQ_ZERO_BUFFER[0].load(Ordering::Relaxed) ^ BARQ_INITIALIZED_BUFFER[0].load(Ordering::Relaxed)
}
