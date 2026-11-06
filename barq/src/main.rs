mod report;

use clap::Parser;
use object::read::elf::ElfFile;
use object::read::Object;
use object::{Endianness, ObjectSection, ObjectSymbol};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "barq",
    about = "Visualize microcontroller Flash and RAM from ELF files"
)]
struct Cli {
    /// Path to the ELF file
    elf: PathBuf,

    /// Output HTML file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Flash memory start address (auto-detected from ELF if not specified)
    #[arg(long)]
    flash_origin: Option<String>,

    /// RAM memory start address (auto-detected from ELF if not specified)
    #[arg(long)]
    ram_origin: Option<String>,
}

/// A symbol mapped to a memory region.
#[derive(Debug, Clone)]
struct MappedSymbol {
    name: String,
    address: u64,
    size: u64,
    section: String,
    kind: SymbolKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SymbolKind {
    Function,
    Data,
    Other,
}

/// A contiguous memory region on the target (Flash or RAM).
#[derive(Debug)]
struct MemoryRegion {
    name: String,
    start: u64,
    size: u64,
    symbols: Vec<MappedSymbol>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MemoryKind {
    Flash,
    Ram,
}

/// Known address mappings used to classify ELF symbol addresses.
/// These ranges are heuristics, not the detected capacity of a specific chip.
const MEMORY_RANGES: &[(std::ops::RangeInclusive<u64>, MemoryKind)] = &[
    // STM32 Flash
    (0x0800_0000..=0x081F_FFFF, MemoryKind::Flash),
    // ESP32-C3 mapped Flash
    (0x4200_0000..=0x427F_FFFF, MemoryKind::Flash),
    // RP2040 Flash (XIP)
    (0x1000_0000..=0x1FFF_FFFF, MemoryKind::Flash),
    // nRF52 Flash
    (0x0000_0000..=0x000F_FFFF, MemoryKind::Flash),
    // STM32 / nRF52 RAM
    (0x2000_0000..=0x2003_FFFF, MemoryKind::Ram),
    // ESP32-C3 DRAM
    (0x3FC8_0000..=0x3FCE_0000, MemoryKind::Ram),
    // ESP32-C3 IRAM
    (0x4037_0000..=0x4037_FFFF, MemoryKind::Ram),
];

fn classify_address(addr: u64) -> Option<MemoryKind> {
    MEMORY_RANGES
        .iter()
        .find(|(range, _)| range.contains(&addr))
        .map(|(_, kind)| *kind)
}

fn parse_elf(
    data: &[u8],
) -> Result<(Vec<MemoryRegion>, Vec<MemoryRegion>), Box<dyn std::error::Error>> {
    let elf = ElfFile::<object::elf::FileHeader32<Endianness>>::parse(data).or_else(|_| {
        ElfFile::<object::elf::FileHeader64<Endianness>>::parse(data)
            .map(|_| panic!("64-bit ELF not typical for MCUs — but could be supported"))
    })?;

    let mut flash_symbols: Vec<MappedSymbol> = Vec::new();
    let mut ram_symbols: Vec<MappedSymbol> = Vec::new();

    // Collect section info for labeling
    let section_names: BTreeMap<u64, String> = elf
        .sections()
        .filter_map(|s| {
            let addr = s.address();
            let name = s.name().ok()?.to_string();
            if addr > 0 {
                Some((addr, name))
            } else {
                None
            }
        })
        .collect();

    // Extract symbols
    for symbol in elf.symbols() {
        let name = match symbol.name() {
            Ok(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };

        let address = symbol.address();
        let size = symbol.size();

        if address == 0 && size == 0 {
            continue;
        }

        let kind = match symbol.kind() {
            object::SymbolKind::Text => SymbolKind::Function,
            object::SymbolKind::Data => SymbolKind::Data,
            _ => SymbolKind::Other,
        };

        // Find which section this symbol belongs to
        let section = section_names
            .range(..=address)
            .next_back()
            .map(|(_, name)| name.clone())
            .unwrap_or_else(|| "unknown".to_string());

        let mapped = MappedSymbol {
            name,
            address,
            size,
            section,
            kind,
        };

        match classify_address(address) {
            Some(MemoryKind::Flash) => flash_symbols.push(mapped),
            Some(MemoryKind::Ram) => ram_symbols.push(mapped),
            None => {
                // Try to classify by section name
                if mapped.section.contains("text")
                    || mapped.section.contains("rodata")
                    || mapped.section.contains("vector")
                {
                    flash_symbols.push(mapped);
                } else if mapped.section.contains("bss")
                    || mapped.section.contains("data")
                    || mapped.section.contains("stack")
                {
                    ram_symbols.push(mapped);
                }
            }
        }
    }

    flash_symbols.sort_by_key(|s| s.address);
    ram_symbols.sort_by_key(|s| s.address);

    let flash_regions = build_regions("Flash", flash_symbols);
    let ram_regions = build_regions("RAM", ram_symbols);

    Ok((flash_regions, ram_regions))
}

fn build_regions(name: &str, symbols: Vec<MappedSymbol>) -> Vec<MemoryRegion> {
    if symbols.is_empty() {
        return vec![];
    }

    // Group symbols by section
    let mut by_section: BTreeMap<String, Vec<MappedSymbol>> = BTreeMap::new();
    for sym in &symbols {
        by_section
            .entry(sym.section.clone())
            .or_default()
            .push(sym.clone());
    }

    let mut regions = Vec::new();
    for (section_name, section_symbols) in by_section {
        let start = section_symbols.first().map(|s| s.address).unwrap_or(0);
        let end = section_symbols
            .iter()
            .map(|s| s.address + s.size)
            .max()
            .unwrap_or(start);

        regions.push(MemoryRegion {
            name: format!("{name}: {section_name}"),
            start,
            size: end - start,
            symbols: section_symbols,
        });
    }

    regions
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let data = fs::read(&cli.elf)?;
    let elf_name = cli
        .elf
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let (flash_regions, ram_regions) = parse_elf(&data)?;

    eprintln!(
        "Flash: {} regions, {} symbols",
        flash_regions.len(),
        flash_regions.iter().map(|r| r.symbols.len()).sum::<usize>()
    );
    eprintln!(
        "RAM:   {} regions, {} symbols",
        ram_regions.len(),
        ram_regions.iter().map(|r| r.symbols.len()).sum::<usize>()
    );

    let html = report::generate_html(&flash_regions, &ram_regions, &elf_name);

    match cli.output {
        Some(path) => {
            fs::write(&path, &html)?;
            eprintln!("Written to {}", path.display());
        }
        None => print!("{html}"),
    }

    Ok(())
}
