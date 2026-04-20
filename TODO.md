# Barq — TODO

## Core improvements

- [ ] Use ELF PT_LOAD segments to detect flash vs ram instead of hardcoded address tables
- [ ] Demangle Rust symbol names (currently shows raw mangled names like `_ZN14stm32f4_blinky...`)
- [ ] Extract crate/module name from demangled symbols for grouping and coloring
- [ ] Handle symbols with size=0 (many linker symbols) — currently they get dropped
- [ ] Show gaps/free space between symbols (unused flash/ram)
- [ ] Show total flash/ram capacity vs used (requires reading linker script or PT_LOAD)

## Visualization

- [ ] Treemap view as alternative to linear block layout
- [ ] Color by crate/module instead of symbol kind
- [ ] Proportional sizing — blocks should reflect actual size relative to total memory
- [ ] Sortable table view alongside the map
- [ ] Click to expand/collapse sections
- [ ] Show address ranges on hover in hex
- [ ] Dark/light theme toggle

## Features

- [ ] Diff mode — compare two ELF files side by side (e.g. before/after a change)
- [ ] cargo-bloat style size attribution by crate
- [ ] Export symbol table as CSV/JSON
- [ ] Support 64-bit ELF (currently panics)
- [ ] Read memory regions from linker script if provided (`--memory-x path`)

## Examples

- [ ] Fix static_mut_refs warnings in both examples (use core::cell or atomic)
- [ ] Add nRF52840 example
- [ ] Add RP2040 example

## Future

- [ ] WASM web app — drag-and-drop ELF file, parse client-side in browser
- [ ] cargo subcommand (`cargo barq`) that builds and visualizes in one step
- [ ] Integration with probe-rs for live memory inspection
