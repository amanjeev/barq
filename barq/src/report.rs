use crate::{MemoryRegion, SymbolKind};

pub(crate) fn generate_html(
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

    let values = [
        ("styles", include_str!("report/style.css").to_string()),
        ("script", include_str!("report/script.js").to_string()),
        ("elf_name", html_escape(elf_name)),
        ("flash_sym_count", flash_sym_count.to_string()),
        ("flash_total", flash_total.to_string()),
        ("ram_sym_count", ram_sym_count.to_string()),
        ("ram_total", ram_total.to_string()),
        ("flash_blocks", flash_blocks),
        ("ram_blocks", ram_blocks),
    ];

    // Expand only template tokens, never tokens inside inserted ELF data or assets.
    let mut parts = include_str!("report/index.html").split("{{");
    let mut html = parts.next().unwrap_or_default().to_string();
    for part in parts {
        let (key, rest) = part.split_once("}}").expect("closed report token");
        let (_, value) = values
            .iter()
            .find(|(name, _)| *name == key)
            .expect("known report token");
        html.push_str(value);
        html.push_str(rest);
    }
    html
}

fn render_memory_blocks(regions: &[MemoryRegion]) -> String {
    let mut html = String::new();
    for region in regions {
        html.push_str(&format!(
            r#"<div class="section-group"><div class="section-label">{} (0x{:08x}, {} bytes)</div>"#,
            html_escape(&region.name), region.start, region.size
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
    if name.chars().count() <= max_len {
        name.to_string()
    } else if max_len == 0 {
        String::new()
    } else {
        format!("{}…", name.chars().take(max_len - 1).collect::<String>())
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MappedSymbol;

    #[test]
    fn report_escapes_elf_data_without_expanding_it_as_template_tokens() {
        let name = "<img src=x onerror=alert(1)> {{script}}";
        let regions = vec![MemoryRegion {
            name: name.to_string(),
            start: 0x0800_0000,
            size: 4,
            symbols: vec![MappedSymbol {
                name: name.to_string(),
                address: 0x0800_0000,
                size: 4,
                section: "\"<section>".to_string(),
                kind: SymbolKind::Function,
            }],
        }];
        let html = generate_html(&regions, &[], name);
        assert!(html.contains(&format!("<title>barq — {}</title>", html_escape(name))));
        assert!(html.contains(&format!("data-name=\"{}\"", html_escape(name))));
        assert!(html.contains("&quot;&lt;section&gt;"));
        assert!(!html.contains("<img"));
        assert!(html.contains(include_str!("report/style.css")));
        assert!(html.contains(include_str!("report/script.js")));
        assert!(!html.contains("tooltip.innerHTML"));
    }

    #[test]
    fn truncation_respects_unicode_character_boundaries() {
        assert_eq!(truncate_name("aé⚡z", 3), "aé…");
        assert_eq!(truncate_name("⚡", 1), "⚡");
        assert_eq!(truncate_name("ab", 1), "…");
        assert_eq!(truncate_name("ab", 0), "");
    }
}
