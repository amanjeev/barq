# barq

Interactive visualization of microcontroller Flash and RAM contents from ELF files.

**barq** (Farsi for "lightning/flash") parses embedded firmware ELF files and generates an interactive HTML page showing what lives in Flash and RAM — every function, every variable, every constant — mapped to its actual address on the microcontroller.

## Why

Existing tools show you parts of the picture:

- `nm` gives you a flat symbol list — no visualization
- `cargo-bloat` gives you size breakdowns — no spatial layout
- [elfcat](https://github.com/rbakbashev/elfcat) shows raw ELF structure — not the MCU memory
- Nordic's memory visualizer shows coarse regions — not symbol-level detail

barq shows you **what's on the microcontroller at boot**, symbol by symbol.

## Usage

```
barq firmware.elf -o output.html
```

Open `output.html` in a browser. You get:

- **Flash panel** — code (.text), constants (.rodata), vector table, interrupt handlers
- **RAM panel** — .bss (zero-initialized), .data (initialized globals), stack/heap info
- **Search** — filter symbols by name
- **Hover tooltips** — address, size, section for each symbol

## Building

```
cd barq
cargo build --release
```

## Example firmware

Two example embedded projects are included for generating test ELF files. Both use the same workload: a blinking LED, a 32-bit counter, a 64-element `u16` sensor buffer, and an eight-element calibration table. Both halt on panic and omit logging. Hardware setup, runtime code, and instruction sets still differ, so their binary sizes will differ.

### STM32F411 (ARM Cortex-M4)

```
cd examples/stm32f4-blinky
rustup target add thumbv7em-none-eabihf
cargo build --release
```

Produces `target/thumbv7em-none-eabihf/release/stm32f4-blinky`

### ESP32-C3 (RISC-V)

```
cd examples/esp32c3-blinky
rustup target add riscv32imc-unknown-none-elf
cargo build --release
```

Produces `target/riscv32imc-unknown-none-elf/release/esp32c3-blinky`

### Visualize

```
barq examples/stm32f4-blinky/target/thumbv7em-none-eabihf/release/stm32f4-blinky -o stm32f4.html
barq examples/esp32c3-blinky/target/riscv32imc-unknown-none-elf/release/esp32c3-blinky -o esp32c3.html
```

## How it works

1. Parses the ELF file using the `object` crate
2. Extracts symbols from the symbol table (.symtab) with their addresses and sizes
3. Classifies each symbol into Flash or RAM based on its address
4. Groups symbols by ELF section (.text, .rodata, .bss, .data)
5. Generates a self-contained HTML file with interactive blocks for each symbol

## Supported targets

barq recognizes memory layouts for:

| Chip family | Flash | RAM |
|-------------|-------|-----|
| STM32 | `0x0800_0000` | `0x2000_0000` |
| ESP32-C3 | `0x4200_0000` | `0x3FC8_0000` |
| nRF52 | `0x0000_0000` | `0x2000_0000` |
| RP2040 | `0x1000_0000` | `0x2000_0000` |

## License

AGPL-3.0-or-later
