# barq

Turn a microcontroller ELF file into an interactive Flash and RAM report:

```sh
barq firmware.elf -o report.html
```

Open `report.html` in a browser. The report is a single file that works offline.

`barq`: ([برق](https://en.wiktionary.org/wiki/%D8%A8%D8%B1%D9%82#Arabic), IPA: /barq/) takes its name from the Persian word for electricity, borrowed from Arabic “lightning.”

barq shows named functions, variables, and constants at their firmware addresses. Use it to inspect memory contents without reading a flat symbol list.

## Install and run

With Rust and Cargo installed:

1. Install barq from the repository root:

   ```sh
   cargo install --locked --path barq
   ```

2. Generate a report from your firmware ELF:

   ```sh
   barq firmware.elf -o report.html
   ```

3. Open `report.html` in your browser.

Omit `-o` to write HTML to stdout. Run `barq --help` for all options.

## Read the report

| Feature | What it shows |
|---|---|
| Flash panel | Symbols classified as Flash, grouped by ELF section |
| RAM panel | Symbols classified as RAM, including code that runs from RAM |
| Search | Address groups containing a matching readable or original symbol name |
| Symbol details | Full name, address, size, and section, with explanations of types, zero sizes, and shared addresses |

### Shared addresses and overlaps

Symbols starting at the same address within a section appear in one block.

| Indicator | Meaning |
|---|---|
| Unique known bytes | Bytes covered by nonzero-size symbol ranges, counted once |
| Shared bytes | Bytes covered by at least two symbol ranges, counted once even if more than two overlap |
| Intersecting address groups | Groups with different starting addresses whose recorded ranges overlap; details show the shared byte count for each pair |
| Unknown (0 recorded) | An entry has no recorded extent. It remains visible but contributes no inferred bytes to coverage or overlap counts. |

### Symbols

| Field | Meaning |
|---|---|
| Name | Rust names are displayed in readable form, such as `stm32f4_blinky::apply_calibration`. Details also retain the original ELF name, such as `_RNv…`. Non-Rust names remain unchanged. |
| Address | Where the symbol is located in the target’s address space, shown in hexadecimal. This is not its offset inside the ELF file. |
| Size | The byte count recorded for the symbol. Zero means no extent is recorded; it does not prove the symbol is useless or that the referenced code occupies no memory. |
| Section | The ELF group containing the symbol, such as `.text` or `.bss`. |

Search accepts both readable and original names, including names inside shared-address groups. CLI exclusions (`--exclude-symbol` and `--exclude-symbol-regex`) match **original ELF names only**; copy them from the details table’s `ELF:` line. Readable names omit Rust hash suffixes; distinct original entries remain separate in group details even if their readable names match.

### Common sections

| Section | Typical contents |
|---|---|
| `.text` | Executable code, usually in Flash |
| `.rodata` | Read-only constants, usually in Flash |
| `.data` | Writable variables with initial values; RAM at runtime, typically initialized from a Flash copy |
| `.bss` | Writable variables zeroed at startup; consumes RAM without storing every zero in the firmware image |
| `.rwtext` / `.trap` | RAM-resident code and trap handling in our ESP32-C3 examples |

Section names and placement depend on the target and linker configuration. barq shows symbols at their runtime addresses; it does not separately account for the Flash initialization copy of `.data`.

### Symbols that need interpretation

| Symbol case | How to interpret it |
|---|---|
| Aliases | Multiple names can share one address without occupying separate memory. In `symbol-edges`, `barq_payload_alias` refers to the same location as `barq_edge_payload`. |
| Zero-size markers | Labels such as `barq_zero_size_marker` identify a location without claiming bytes. Interrupt-handler aliases can also have zero recorded size. |
| Internal labels | Names such as `.L0`, `$t`, `$d`, and `$xrv…` are toolchain bookkeeping. Exclude specific names or patterns when they clutter the report. |
| Missing source names | Optimization can inline functions, combine data, or remove unused items. A source declaration is not guaranteed to have its own ELF symbol. |

## Try the example firmware

Run the commands below from the repository root.

```sh
cargo build --locked --release --bins
```

Replace the blinky filename in the commands below with the workload name to generate its report.

### Additional workloads

Each architecture also provides these binaries, using shared workload source:

| Binary | Purpose |
|---|---|
| `memory-sections` | Known-size writable and read-only buffers in `.bss`, `.data`, and `.rodata` |
| `symbol-edges` | Zero-size markers, shared-address aliases, long names, and filter fixtures |
| `large-firmware` | 256 functions and 256 counters for search and navigation |


See the [example guide](https://github.com/amanjeev/barq/blob/main/examples/README.md) for expected symbols and filter commands.

### STM32F411: ARM Cortex-M4

1. Build the firmware:

   ```sh
   (cd examples/stm32f4-blinky && cargo build --locked --release --bin stm32f4-blinky)
   ```

2. Generate its report:

   ```sh
   barq examples/stm32f4-blinky/target/thumbv7em-none-eabihf/release/stm32f4-blinky -o stm32f4.html
   ```

3. Open `stm32f4.html` in a browser.

### ESP32-C3: RISC-V

1. Build the firmware:

   ```sh
   (cd examples/esp32c3-blinky && cargo build --locked --release --bin esp32c3-blinky)
   ```

2. Generate its report:

   ```sh
   barq examples/esp32c3-blinky/target/riscv32imc-unknown-none-elf/release/esp32c3-blinky -o esp32c3.html
   ```

3. Open `esp32c3.html` in a browser.

## How it works

1. Read the 32-bit ELF with the `object` crate.
2. Extract named symbols and their actual ELF section membership.
3. Classify symbols as Flash or RAM using known address ranges, with section-name fallbacks.
4. Group symbols by section and calculate their address spans.
5. Embed the HTML template, CSS, JavaScript, and symbol data in one report.

Rust symbol names are demangled for display; original ELF names remain available in details. ELF files with stripped symbols provide less detail. 64-bit ELF files are not supported.

## License

AGPL-3.0-or-later
