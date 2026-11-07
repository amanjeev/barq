mod overlap;

use crate::{MappedSymbol, MemoryRegion, SymbolKind};
use overlap::{coverage, group_symbols, range};
use std::cmp::Reverse;

pub(crate) fn generate_html(
    flash_regions: &[MemoryRegion],
    ram_regions: &[MemoryRegion],
    elf_name: &str,
) -> String {
    let flash_total: u64 = flash_regions.iter().map(|r| r.size).sum();
    let ram_total: u64 = ram_regions.iter().map(|r| r.size).sum();
    let flash_sym_count: usize = flash_regions.iter().map(|r| r.symbols.len()).sum();
    let ram_sym_count: usize = ram_regions.iter().map(|r| r.symbols.len()).sum();

    let flash_blocks = render_memory_blocks(flash_regions, "flash");
    let ram_blocks = render_memory_blocks(ram_regions, "ram");

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

fn display_name(name: &str) -> String {
    rustc_demangle::try_demangle(name)
        .map(|symbol| format!("{symbol:#}"))
        .unwrap_or_else(|_| name.to_string())
}

fn kind_names(kind: SymbolKind) -> (&'static str, &'static str) {
    match kind {
        SymbolKind::Function => ("function", "Function"),
        SymbolKind::Data => ("data", "Data"),
        SymbolKind::Other => ("other", "Other"),
    }
}

fn representative<'a>(symbols: &[&'a MappedSymbol]) -> &'a MappedSymbol {
    symbols
        .iter()
        .copied()
        .min_by_key(|symbol| {
            let kind = match symbol.kind {
                SymbolKind::Function => 0,
                SymbolKind::Data => 1,
                SymbolKind::Other => 2,
            };
            (Reverse(symbol.size), kind, &symbol.name)
        })
        .expect("nonempty address group")
}

fn render_memory_blocks(regions: &[MemoryRegion], panel: &str) -> String {
    let mut html = String::new();
    let mut group_number = 0;
    for region in regions {
        let groups = group_symbols(&region.symbols);
        let covered = coverage(region.symbols.iter().map(range));
        html.push_str(&format!(
            r#"<div class="section-group"><h3 class="section-label">{} (0x{:08x}, {} bytes span)</h3><p class="coverage">{} unique known bytes; {} bytes covered by multiple symbols. {} symbol entries in {} address groups.</p>"#,
            html_escape(&region.name), region.start, region.size,
            covered.known, covered.shared, region.symbols.len(), groups.len(),
        ));
        for group in &groups {
            let sym = representative(&group.symbols);
            let (kind_class, kind_label) = kind_names(sym.kind);
            let readable_name = display_name(&sym.name);
            let mut accessible_name = format!(
                "{}. {kind_label}. Address 0x{:08x}. Size {} bytes. Section {}. Open details.",
                readable_name, sym.address, sym.size, sym.section
            );
            if group.symbols.len() > 1 {
                accessible_name.push_str(&format!(
                    " {} symbol entries share this address.",
                    group.symbols.len()
                ));
            }
            if group.shared_bytes > 0 {
                accessible_name.push_str(&format!(
                    " {} known bytes shared with other symbols.",
                    group.shared_bytes
                ));
            }
            let width = (group.known_size as f64).sqrt().max(4.0) * 15.0;
            let id = format!("{panel}-symbols-{group_number}");
            group_number += 1;
            html.push_str(&format!(
                r#"<button type="button" class="symbol-block {kind_class}" style="--symbol-width:{width:.0}px" aria-label="{}" aria-haspopup="dialog" aria-controls="symbol-dialog" data-name="{}" data-display-name="{}" data-addr="{}" data-size="{}" data-section="{}" data-members="{id}" data-count="{}" data-known="{}" data-shared="{}"><span class="symbol-name">{}</span><span class="symbol-kind">{kind_label}"#,
                html_escape(&accessible_name), html_escape(&sym.name), html_escape(&readable_name), sym.address, sym.size,
                html_escape(&sym.section), group.symbols.len(), group.known_size, group.shared_bytes,
                html_escape(&readable_name),
            ));
            if group.symbols.len() > 1 {
                html.push_str(&format!(" · {} entries", group.symbols.len()));
            }
            html.push_str("</span>");
            if group.shared_bytes > 0 {
                html.push_str(&format!(
                    r#"<span class="overlap-badge">{} shared bytes</span>"#,
                    group.shared_bytes
                ));
            }
            html.push_str(&format!(r#"</button><template id="{id}">"#));
            // Inert HTML retains every name and size, without creating a focus
            // stop for each alias. ELF text is escaped, never embedded as JS.
            for member in &group.symbols {
                let (_, kind) = kind_names(member.kind);
                html.push_str(&format!(
                    r#"<span data-name="{}" data-size="{}" data-kind="{kind}" data-display-name="{}"></span>"#,
                    html_escape(&member.name),
                    member.size,
                    html_escape(&display_name(&member.name)),
                ));
            }
            for &(index, bytes) in &group.overlaps {
                let other = &groups[index];
                html.push_str(&format!(
                    r#"<span data-overlap-name="{}" data-overlap-addr="{}" data-overlap-bytes="{bytes}" data-overlap-display-name="{}"></span>"#,
                    html_escape(&representative(&other.symbols).name), other.address,
                    html_escape(&display_name(&representative(&other.symbols).name)),
                ));
            }
            html.push_str("</template>");
        }
        html.push_str("</div>");
    }
    html
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
        // Full names and metadata remain available even when the visible name
        // is shortened. Keep user-supplied text escaped in accessible labels too.
        assert!(html.contains(&format!(
            "aria-label=\"{}. Function. Address 0x08000000. Size 4 bytes. Section {}. Open details.\"",
            html_escape(name), html_escape("\"<section>")
        )));
        assert!(html.contains("<button type=\"button\" class=\"symbol-block function\""));
        assert!(html.contains("aria-haspopup=\"dialog\" aria-controls=\"symbol-dialog\""));
        assert!(html.contains("&quot;&lt;section&gt;"));
        assert!(!html.contains("<img"));
        assert!(html.contains(include_str!("report/style.css")));
        assert!(html.contains(include_str!("report/script.js")));
        assert!(!html.contains("tooltip.innerHTML"));
    }

    #[test]
    fn report_provides_named_dialog_search_feedback_and_landmarks() {
        let html = generate_html(&[], &[], "empty.elf");
        assert!(html.contains("<main id=\"main-content\">"));
        assert!(html.contains("<label for=\"search\">Search symbols</label>"));
        assert!(html.contains("role=\"status\" aria-live=\"polite\""));
        assert!(html.contains("<dialog id=\"symbol-dialog\" aria-labelledby=\"dialog-title\">"));
        assert!(html.contains("id=\"dialog-title\""));
        assert!(html.contains("id=\"close-dialog\" autofocus"));
    }

    fn fixture_region(name: &str, symbols: Vec<MappedSymbol>) -> MemoryRegion {
        MemoryRegion {
            name: name.to_string(),
            start: 0x1000,
            size: 24,
            symbols,
        }
    }

    fn fixture_symbol(name: &str, address: u64, size: u64) -> MappedSymbol {
        MappedSymbol {
            name: name.to_string(),
            address,
            size,
            section: ".text".to_string(),
            kind: SymbolKind::Function,
        }
    }

    #[test]
    fn shared_address_block_retains_all_entries_and_prefers_sized_name() {
        let region = fixture_region(
            "Flash: .text",
            vec![
                fixture_symbol("alias<&", 0x1000, 0),
                fixture_symbol("handler", 0x1000, 8),
                fixture_symbol("marker", 0x1000, 0),
            ],
        );
        let html = generate_html(&[region], &[], "test.elf");
        assert_eq!(
            html.matches("<button type=\"button\" class=\"symbol-block")
                .count(),
            1
        );
        assert!(html.contains("<span class=\"symbol-name\">handler</span>"));
        assert!(html.contains("data-count=\"3\" data-known=\"8\" data-shared=\"0\""));
        assert!(html.contains("<span data-name=\"alias&lt;&amp;\" data-size=\"0\""));
        assert!(html.contains("<span data-name=\"marker\" data-size=\"0\""));
        assert!(html.contains("8 unique known bytes; 0 bytes covered by multiple symbols"));
    }

    #[test]
    fn partial_overlaps_link_both_groups_and_do_not_cross_sections() {
        let first = fixture_region(
            "Flash: .text",
            vec![
                fixture_symbol("first", 0x1000, 16),
                fixture_symbol("second", 0x1008, 16),
            ],
        );
        let other = fixture_region(
            "Flash: .other",
            vec![fixture_symbol("other_section", 0x1000, 16)],
        );
        let html = generate_html(&[first, other], &[], "test.elf");
        assert_eq!(
            html.matches("<button type=\"button\" class=\"symbol-block")
                .count(),
            3
        );
        assert_eq!(html.matches("data-overlap-bytes=\"8\"").count(), 2);
        assert!(html.contains("24 unique known bytes; 8 bytes covered by multiple symbols"));
        assert!(html.contains("16 unique known bytes; 0 bytes covered by multiple symbols"));
        assert!(!html.contains("data-overlap-name=\"other_section\""));
    }

    #[test]
    fn demangles_legacy_and_v0_names_without_hash_suffixes() {
        assert_eq!(display_name("_ZN3foo3bar17h05af221e174051e9E"), "foo::bar");
        assert_eq!(
            display_name("_RNvCsj7b3fXFyzpb_14stm32f4_blinky17apply_calibration"),
            "stm32f4_blinky::apply_calibration"
        );
    }

    #[test]
    fn keeps_non_rust_and_malformed_names_unchanged() {
        for name in [
            "DefaultHandler",
            ".L0 ",
            "$d",
            "_Rinvalid",
            "_ZNbroken",
            "aé⚡z",
            "_Z3foov",
        ] {
            assert_eq!(display_name(name), name);
        }
    }

    #[test]
    fn escapes_html_characters_introduced_by_demangling() {
        let original = "_ZN4test9$LT$T$GT$E";
        assert_eq!(display_name(original), "test::<T>");
        let region = fixture_region("Flash: .text", vec![fixture_symbol(original, 0x1000, 8)]);
        let html = generate_html(&[region], &[], "test.elf");
        assert!(html.contains("<span class=\"symbol-name\">test::&lt;T&gt;</span>"));
        assert!(html.contains("data-display-name=\"test::&lt;T&gt;\""));
        assert!(!html.contains("test::<T>"));
        assert!(html.contains(&format!("data-name=\"{original}\"")));
    }

    #[test]
    fn readable_names_do_not_replace_or_merge_original_identities() {
        let originals = [
            "_ZN3foo3bar17h05af221e174051e9E",
            "_ZN3foo3bar17h1111111111111111E",
        ];
        let region = fixture_region(
            "Flash: .text",
            originals
                .iter()
                .map(|name| fixture_symbol(name, 0x1000, 8))
                .collect(),
        );
        let html = generate_html(&[region], &[], "test.elf");
        assert!(html.contains("<span class=\"symbol-name\">foo::bar</span>"));
        assert!(html.contains("aria-label=\"foo::bar. Function."));
        for original in originals {
            assert!(html.contains(&format!("data-name=\"{original}\"")));
        }
        assert!(html.contains("data-count=\"2\""));
        assert_eq!(html.matches("data-display-name=\"foo::bar\"").count(), 3);
    }
}
