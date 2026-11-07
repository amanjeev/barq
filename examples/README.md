# Firmware workloads

Build all four workloads for one architecture from the repository root:

```sh
(cd examples/stm32f4-blinky && cargo build --locked --release --bins)
```

For ESP32-C3, replace `stm32f4-blinky` with `esp32c3-blinky`. Each package’s toolchain file selects its compiler and target. No hardware is needed to build ELF files or generate reports.

## Choose a workload

| Binary | What to look for |
|---|---|
| `stm32f4-blinky` / `esp32c3-blinky` | Baseline: LED blinking, counter, sensor buffer, and calibration data |
| `memory-sections` | `BARQ_ZERO_BUFFER`: 512 bytes in `.bss`; `BARQ_INITIALIZED_BUFFER`: 128 bytes in `.data`; `BARQ_LOOKUP_TABLE`: 256 bytes in `.rodata` |
| `symbol-edges` | A zero-size marker and alias sharing the payload address, two partially overlapping eight-byte ranges, a long name, and three functions for filter experiments |
| `large-firmware` | 256 separate stage functions, 256 writable counters, and a function-pointer table |

The three additional workloads share source in `examples/workloads/`. The architecture-specific binaries supply startup code.

## Generate a report

With barq installed:

1. Build the binaries using the command above.
2. Run barq on the workload you want:

   ```sh
   barq examples/stm32f4-blinky/target/thumbv7em-none-eabihf/release/memory-sections -o memory-sections.html
   ```

3. Open `memory-sections.html` in a browser.

ESP32-C3 ELF files live under `examples/esp32c3-blinky/target/riscv32imc-unknown-none-elf/release/`.

## Try the symbol filters

Generate an unfiltered `symbol-edges` report, then compare it with:

```sh
barq examples/stm32f4-blinky/target/thumbv7em-none-eabihf/release/symbol-edges \
  --exclude-symbol barq_filter_literal \
  --exclude-symbol-regex '^barq_filter_group_' \
  -o symbol-edges-filtered.html
```

The filtered report hides the three `barq_filter_*` functions. It keeps `barq_zero_size_marker`, `barq_payload_alias`, and their four-byte `barq_edge_payload`.

## Check all workloads

Run from the repository root. Install both toolchains and targets first:

```sh
rustup toolchain install stable --profile minimal --target thumbv7em-none-eabihf
rustup toolchain install nightly --profile minimal --component rust-src --target riscv32imc-unknown-none-elf
```

Then build and validate the site:

```sh
cargo xtask site
```

This builds both architectures, validates the fixtures, and generates all eight reports in `dist/`.