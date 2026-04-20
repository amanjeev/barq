use clap::Parser;
use object::read::elf::ElfFile;
use object::read::Object;
use object::{Endianness, ObjectSection, ObjectSymbol};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "barq", about = "Visualize microcontroller Flash and RAM from ELF files")]
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

/// Known memory layouts for common microcontrollers.
/// We detect these from the ELF load addresses.
fn classify_address(addr: u64) -> Option<&'static str> {
    // Order matters: more specific ranges first, no overlaps.
    // STM32 Flash
    if (0x0800_0000..=0x081F_FFFF).contains(&addr) {
        return Some("flash");
    }
    // ESP32-C3 Flash (IROM/DROM mapped)
    if (0x4200_0000..=0x427F_FFFF).contains(&addr) {
        return Some("flash");
    }
    // RP2040 Flash (XIP)
    if (0x1000_0000..=0x1FFF_FFFF).contains(&addr) {
        return Some("flash");
    }
    // nRF52 Flash (starts at 0x0)
    if (0x0000_0000..=0x000F_FFFF).contains(&addr) {
        return Some("flash");
    }
    // STM32 / nRF52 RAM (overlapping range, both at 0x2000_0000)
    if (0x2000_0000..=0x2003_FFFF).contains(&addr) {
        return Some("ram");
    }
    // ESP32-C3 DRAM
    if (0x3FC8_0000..=0x3FCE_0000).contains(&addr) {
        return Some("ram");
    }
    // ESP32-C3 IRAM
    if (0x4037_0000..=0x4037_FFFF).contains(&addr) {
        return Some("ram");
    }
    None
}

fn parse_elf(data: &[u8]) -> Result<(Vec<MemoryRegion>, Vec<MemoryRegion>), Box<dyn std::error::Error>> {
    let elf = ElfFile::<object::elf::FileHeader32<Endianness>>::parse(data)
        .or_else(|_| {
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
            if addr > 0 { Some((addr, name)) } else { None }
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
            Some("flash") => flash_symbols.push(mapped),
            Some("ram") => ram_symbols.push(mapped),
            _ => {
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

fn generate_html(
    flash_regions: &[MemoryRegion],
    ram_regions: &[MemoryRegion],
    elf_name: &str,
) -> String {
    let flash_total: u64 = flash_regions.iter().map(|r| r.size).sum();
    let ram_total: u64 = ram_regions.iter().map(|r| r.size).sum();
    let flash_sym_count: usize = flash_regions.iter().map(|r| r.symbols.len()).sum();
    let ram_sym_count: usize = ram_regions.iter().map(|r| r.symbols.len()).sum();

    let flash_blocks = render_memory_blocks(flash_regions);
    let ram_blocks = render_memory_blocks(ram_regions);

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>barq — {elf_name}</title>
<style>
  * {{ margin: 0; padding: 0; box-sizing: border-box; }}
  body {{
    font-family: 'SF Mono', 'Cascadia Code', 'Fira Code', monospace;
    background: #0d1117;
    color: #c9d1d9;
    padding: 2rem;
  }}
  h1 {{ color: #58a6ff; margin-bottom: 0.5rem; }}
  .subtitle {{ color: #8b949e; margin-bottom: 2rem; }}
  .memory-container {{
    display: flex;
    gap: 2rem;
    flex-wrap: wrap;
  }}
  .memory-panel {{
    flex: 1;
    min-width: 400px;
    background: #161b22;
    border: 1px solid #30363d;
    border-radius: 8px;
    padding: 1.5rem;
  }}
  .memory-panel h2 {{
    color: #f0883e;
    margin-bottom: 0.5rem;
  }}
  .memory-stats {{
    color: #8b949e;
    font-size: 0.85rem;
    margin-bottom: 1rem;
  }}
  .memory-map {{
    display: flex;
    flex-direction: column;
    gap: 2px;
  }}
  .section-group {{
    margin-bottom: 0.5rem;
  }}
  .section-label {{
    font-size: 0.75rem;
    color: #8b949e;
    padding: 0.25rem 0;
  }}
  .symbol-block {{
    display: inline-block;
    padding: 4px 8px;
    margin: 1px;
    border-radius: 3px;
    font-size: 0.7rem;
    cursor: pointer;
    transition: transform 0.1s, box-shadow 0.1s;
    position: relative;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }}
  .symbol-block:hover {{
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(0,0,0,0.4);
    z-index: 10;
  }}
  .symbol-block.function {{ background: #1f3a5f; color: #58a6ff; border: 1px solid #264773; }}
  .symbol-block.data {{ background: #2a1f3f; color: #bc8cff; border: 1px solid #3b2a5f; }}
  .symbol-block.other {{ background: #1f2f1f; color: #7ee787; border: 1px solid #2a4a2a; }}
  .tooltip {{
    display: none;
    position: fixed;
    background: #1c2128;
    border: 1px solid #444c56;
    border-radius: 6px;
    padding: 0.75rem;
    font-size: 0.8rem;
    z-index: 100;
    max-width: 400px;
    pointer-events: none;
  }}
  .tooltip.visible {{ display: block; }}
  .tooltip .label {{ color: #f0883e; font-weight: bold; }}
  .tooltip .addr {{ color: #8b949e; }}
  .tooltip .size {{ color: #7ee787; }}
  .search-box {{
    width: 100%;
    padding: 0.5rem;
    background: #0d1117;
    border: 1px solid #30363d;
    border-radius: 4px;
    color: #c9d1d9;
    font-family: inherit;
    margin-bottom: 1rem;
  }}
  .search-box:focus {{ outline: none; border-color: #58a6ff; }}
  .legend {{
    display: flex;
    gap: 1rem;
    margin-bottom: 1.5rem;
    font-size: 0.85rem;
  }}
  .legend-item {{
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }}
  .legend-swatch {{
    width: 14px;
    height: 14px;
    border-radius: 3px;
  }}
  .hidden {{ display: none !important; }}
</style>
</head>
<body>

<h1>barq</h1>
<p class="subtitle">{elf_name}</p>

<div class="legend">
  <div class="legend-item">
    <div class="legend-swatch" style="background:#1f3a5f;border:1px solid #264773;"></div>
    Function
  </div>
  <div class="legend-item">
    <div class="legend-swatch" style="background:#2a1f3f;border:1px solid #3b2a5f;"></div>
    Data
  </div>
  <div class="legend-item">
    <div class="legend-swatch" style="background:#1f2f1f;border:1px solid #2a4a2a;"></div>
    Other
  </div>
</div>

<input type="text" class="search-box" placeholder="Search symbols..." id="search">

<div class="memory-container">
  <div class="memory-panel">
    <h2>Flash</h2>
    <div class="memory-stats">{flash_sym_count} symbols — {flash_total} bytes total</div>
    <div class="memory-map" id="flash-map">
      {flash_blocks}
    </div>
  </div>
  <div class="memory-panel">
    <h2>RAM</h2>
    <div class="memory-stats">{ram_sym_count} symbols — {ram_total} bytes total</div>
    <div class="memory-map" id="ram-map">
      {ram_blocks}
    </div>
  </div>
</div>

<div class="tooltip" id="tooltip"></div>

<script>
const tooltip = document.getElementById('tooltip');

document.querySelectorAll('.symbol-block').forEach(el => {{
  el.addEventListener('mouseenter', e => {{
    const name = el.dataset.name;
    const addr = el.dataset.addr;
    const size = el.dataset.size;
    const section = el.dataset.section;
    tooltip.innerHTML = `<div class="label">${{name}}</div>
      <div class="addr">Address: 0x${{parseInt(addr).toString(16).padStart(8, '0')}}</div>
      <div class="size">Size: ${{size}} bytes</div>
      <div class="addr">Section: ${{section}}</div>`;
    tooltip.classList.add('visible');
  }});

  el.addEventListener('mousemove', e => {{
    tooltip.style.left = (e.clientX + 12) + 'px';
    tooltip.style.top = (e.clientY + 12) + 'px';
  }});

  el.addEventListener('mouseleave', () => {{
    tooltip.classList.remove('visible');
  }});
}});

document.getElementById('search').addEventListener('input', e => {{
  const query = e.target.value.toLowerCase();
  document.querySelectorAll('.symbol-block').forEach(el => {{
    const name = el.dataset.name.toLowerCase();
    if (query === '' || name.includes(query)) {{
      el.classList.remove('hidden');
    }} else {{
      el.classList.add('hidden');
    }}
  }});
}});
</script>

</body>
</html>"#
    )
}

fn render_memory_blocks(regions: &[MemoryRegion]) -> String {
    let mut html = String::new();
    for region in regions {
        html.push_str(&format!(
            r#"<div class="section-group"><div class="section-label">{} (0x{:08x}, {} bytes)</div>"#,
            region.name, region.start, region.size
        ));
        for sym in &region.symbols {
            let kind_class = match sym.kind {
                SymbolKind::Function => "function",
                SymbolKind::Data => "data",
                SymbolKind::Other => "other",
            };
            // Width proportional to size, minimum 60px
            let width = (sym.size as f64).sqrt().max(4.0) * 15.0;
            html.push_str(&format!(
                r#"<span class="symbol-block {kind_class}" style="max-width:{width:.0}px" data-name="{}" data-addr="{}" data-size="{}" data-section="{}">{}</span>"#,
                html_escape(&sym.name),
                sym.address,
                sym.size,
                html_escape(&sym.section),
                html_escape(&truncate_name(&sym.name, 30)),
            ));
        }
        html.push_str("</div>");
    }
    html
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}…", &name[..max_len - 1])
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
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

    let html = generate_html(&flash_regions, &ram_regions, &elf_name);

    match cli.output {
        Some(path) => {
            fs::write(&path, &html)?;
            eprintln!("Written to {}", path.display());
        }
        None => print!("{html}"),
    }

    Ok(())
}
