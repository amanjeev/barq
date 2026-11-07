//! Explicit ELF fixtures: a zero-size marker, an alias, long names, and filter groups.
use core::hint::black_box;

// Define the marker and alias alongside their payload so the assembler retains
// them at the same address. Only the payload has a recorded size.
core::arch::global_asm!(
    ".section .rodata.barq_edge_payload,\"a\"",
    ".global barq_zero_size_marker",
    "barq_zero_size_marker:",
    ".global barq_edge_payload",
    ".type barq_edge_payload, %object",
    "barq_edge_payload:",
    ".byte 17, 34, 51, 68",
    ".size barq_edge_payload, .-barq_edge_payload",
    ".global barq_payload_alias",
    ".set barq_payload_alias, barq_edge_payload",
    ".size barq_payload_alias, 0",
    // Two eight-byte ranges, four bytes apart: twelve unique bytes, four shared.
    ".section .rodata.barq_overlap_payload,\"a\"",
    ".global barq_overlap_first",
    ".type barq_overlap_first, %object",
    "barq_overlap_first:",
    ".byte 1, 2, 3, 4",
    ".global barq_overlap_second",
    ".type barq_overlap_second, %object",
    "barq_overlap_second:",
    ".byte 5, 6, 7, 8, 9, 10, 11, 12",
    ".size barq_overlap_first, 8",
    ".size barq_overlap_second, 8",
    ".text",
);

extern "C" {
    static barq_edge_payload: u8;
    static barq_overlap_first: u8;
}

#[no_mangle]
pub static BARQ_A_DELIBERATELY_LONG_SYMBOL_NAME_FOR_TESTING_TRUNCATION_AND_FULL_NAME_TOOLTIPS:
    [u8; 48] = [42; 48];

#[no_mangle]
#[inline(never)]
pub fn barq_filter_literal(input: u32) -> u32 {
    black_box(input).wrapping_add(3)
}

#[no_mangle]
#[inline(never)]
pub fn barq_filter_group_one(input: u32) -> u32 {
    black_box(input).rotate_left(5)
}

#[no_mangle]
#[inline(never)]
pub fn barq_filter_group_two(input: u32) -> u32 {
    black_box(input).wrapping_mul(17)
}

pub fn step(tick: u32) -> u32 {
    // SAFETY: this immutable byte is defined in the assembly above and lives
    // in allocated read-only memory. Volatile access retains the ELF fixture.
    let payload = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(barq_edge_payload)) };
    // SAFETY: the assembly fixture above defines this immutable, allocated byte.
    let overlapping = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(barq_overlap_first)) };
    let long_name_data = black_box(
        &BARQ_A_DELIBERATELY_LONG_SYMBOL_NAME_FOR_TESTING_TRUNCATION_AND_FULL_NAME_TOOLTIPS,
    );
    barq_filter_group_two(barq_filter_group_one(barq_filter_literal(tick)))
        ^ u32::from(payload)
        ^ u32::from(overlapping)
        ^ u32::from(long_name_data[tick as usize % long_name_data.len()])
}
