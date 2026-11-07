use object::{Object, ObjectSection, ObjectSymbol, SymbolKind};
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const TARGETS: &[(&str, &str)] = &[
    ("stm32f4", "thumbv7em-none-eabihf"),
    ("esp32c3", "riscv32imc-unknown-none-elf"),
];
const WORKLOADS: &[&str] = &["memory-sections", "symbol-edges", "large-firmware"];

fn main() {
    if let Err(error) = run() {
        eprintln!("xtask: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [command] if command == "site" => build_site(),
        [command] if command == "--help" || command == "-h" => {
            println!("Usage: cargo xtask site\n\nBuild and validate all example reports in dist/.");
            Ok(())
        }
        _ => Err("Usage: cargo xtask site".into()),
    }
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask is in the repository")
        .to_path_buf()
}

fn successful(command: &mut Command) -> Result<()> {
    let status = command.status()?;
    check(
        status.success(),
        format!("{command:?} failed with {status}"),
    )
}

fn captured(command: &mut Command) -> Result<Output> {
    let output = command.output()?;
    check(
        output.status.success(),
        format!(
            "{command:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ),
    )?;
    Ok(output)
}

fn check(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(message.into().into())
    }
}

fn build_package(directory: &Path, bins: bool, toolchain: &str) -> Result<()> {
    // `cargo run` exports its host toolchain to child processes. Use rustup
    // explicitly so a stable host build can still build the nightly ESP target.
    let mut command = Command::new("rustup");
    command.args(["run", toolchain, "cargo"]);
    command
        .current_dir(directory)
        .args(["build", "--locked", "--release"]);
    // Keep artifact paths predictable even when CARGO_TARGET_DIR is set.
    command.arg("--target-dir").arg(directory.join("target"));
    if bins {
        command.arg("--bins");
    }
    successful(&mut command)
}

fn build_site() -> Result<()> {
    let root = root();
    build_package(&root.join("barq"), false, "stable")?;
    for (chip, _) in TARGETS {
        let toolchain = if *chip == "esp32c3" {
            "nightly"
        } else {
            "stable"
        };
        build_package(
            &root.join(format!("examples/{chip}-blinky")),
            true,
            toolchain,
        )?;
    }
    let barq = root.join(format!(
        "barq/target/release/barq{}",
        std::env::consts::EXE_SUFFIX
    ));
    // Validate every fixture before replacing any website content.
    for (chip, target) in TARGETS {
        validate(&barq, &elf_directory(&root, chip, target))?;
        println!("{chip}: section sizes, aliases, zero-size markers, filters, and 256 functions/counters verified");
    }
    let dist = root.join("dist");
    fs::create_dir_all(&dist)?;
    fs::copy(root.join("site/index.html"), dist.join("index.html"))?;
    for (chip, target) in TARGETS {
        let directory = elf_directory(&root, chip, target);
        generate(
            &barq,
            &directory.join(format!("{chip}-blinky")),
            &dist.join(format!("{chip}.html")),
        )?;
        for workload in WORKLOADS {
            generate(
                &barq,
                &directory.join(workload),
                &dist.join(format!("{chip}-{workload}.html")),
            )?;
        }
    }
    fs::write(dist.join(".nojekyll"), "")?;
    println!(
        "Site ready: {} (landing page and eight reports)",
        dist.display()
    );
    Ok(())
}

fn elf_directory(root: &Path, chip: &str, target: &str) -> PathBuf {
    root.join(format!("examples/{chip}-blinky/target/{target}/release"))
}

fn generate(barq: &Path, elf: &Path, destination: &Path) -> Result<()> {
    successful(Command::new(barq).arg(elf).arg("-o").arg(destination))
}

#[derive(Debug)]
struct Symbol {
    address: u64,
    size: u64,
    kind: SymbolKind,
    section: Option<String>,
}

fn symbols(path: &Path) -> Result<BTreeMap<String, Symbol>> {
    let bytes = fs::read(path)?;
    let elf = object::File::parse(bytes.as_slice())?;
    let mut symbols = BTreeMap::new();
    for symbol in elf.symbols() {
        let name = symbol.name()?;
        if name.is_empty() {
            continue;
        }
        let section = symbol
            .section_index()
            .map(|index| {
                elf.section_by_index(index)
                    .and_then(|section| section.name())
                    .map(str::to_owned)
            })
            .transpose()?;
        symbols.insert(
            name.to_string(),
            Symbol {
                address: symbol.address(),
                size: symbol.size(),
                kind: symbol.kind(),
                section,
            },
        );
    }
    Ok(symbols)
}

// Read the data-name attributes emitted by barq's report renderer. This is a
// contract check for our generated markup, not a parser for arbitrary HTML.
fn report_names(html: &str) -> BTreeSet<String> {
    html.split(" data-name=\"")
        .skip(1)
        .filter_map(|part| part.split_once('"').map(|(name, _)| name))
        .map(|name| {
            name.replace("&quot;", "\"")
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
        })
        .collect()
}

fn report(barq: &Path, elf: &Path, options: &[&str]) -> Result<BTreeSet<String>> {
    let output = captured(Command::new(barq).arg(elf).args(options))?;
    Ok(report_names(&String::from_utf8(output.stdout)?))
}

fn require_symbol<'a>(
    symbols: &'a BTreeMap<String, Symbol>,
    name: &str,
    elf: &Path,
) -> Result<&'a Symbol> {
    symbols
        .get(name)
        .ok_or_else(|| format!("{}: missing symbol {name}", elf.display()).into())
}

fn require_names(names: &BTreeSet<String>, expected: &[&str], elf: &Path) -> Result<()> {
    for name in expected {
        check(
            names.contains(*name),
            format!("{}: report missing {name}", elf.display()),
        )?;
    }
    Ok(())
}

fn validate(barq: &Path, directory: &Path) -> Result<()> {
    let elf = directory.join("memory-sections");
    let memory = symbols(&elf)?;
    let names = report(barq, &elf, &[])?;
    for (name, section, size) in [
        ("BARQ_ZERO_BUFFER", ".bss", 512),
        ("BARQ_INITIALIZED_BUFFER", ".data", 128),
        ("BARQ_LOOKUP_TABLE", ".rodata", 256),
    ] {
        let symbol = require_symbol(&memory, name, &elf)?;
        check(
            symbol.section.as_deref() == Some(section) && symbol.size == size,
            format!(
                "{}: {name} should have {size} bytes in {section}, found {symbol:?}",
                elf.display()
            ),
        )?;
        require_names(&names, &[name], &elf)?;
    }

    let elf = directory.join("symbol-edges");
    let edges = symbols(&elf)?;
    let payload = require_symbol(&edges, "barq_edge_payload", &elf)?;
    let first = require_symbol(&edges, "barq_overlap_first", &elf)?;
    let second = require_symbol(&edges, "barq_overlap_second", &elf)?;
    check(
        first.size == 8 && second.size == 8 && second.address == first.address + 4,
        format!(
            "{}: partial-overlap fixture must share four of twelve bytes",
            elf.display()
        ),
    )?;
    for name in ["barq_zero_size_marker", "barq_payload_alias"] {
        let symbol = require_symbol(&edges, name, &elf)?;
        check(
            symbol.size == 0 && symbol.address == payload.address,
            format!(
                "{}: {name} must have size zero and share the payload address",
                elf.display()
            ),
        )?;
    }
    let output = captured(Command::new(barq).arg(&elf))?;
    let html = String::from_utf8(output.stdout)?;
    let names = report_names(&html);
    for (name, address) in [
        ("barq_overlap_first", first.address),
        ("barq_overlap_second", second.address),
    ] {
        check(
            html.contains(&format!(
                "data-overlap-name=\"{name}\" data-overlap-addr=\"{address}\" data-overlap-bytes=\"4\""
            )),
            format!("{}: report missing four-byte overlap link to {name}", elf.display()),
        )?;
    }
    let without_alias = report(barq, &elf, &["--exclude-symbol", "barq_payload_alias"])?;
    check(
        !without_alias.contains("barq_payload_alias"),
        "excluded alias remains in group",
    )?;
    require_names(
        &without_alias,
        &["barq_edge_payload", "barq_zero_size_marker"],
        &elf,
    )?;
    require_names(
        &names,
        &[
            "barq_zero_size_marker",
            "barq_payload_alias",
            "barq_edge_payload",
            "barq_overlap_first",
            "barq_overlap_second",
        ],
        &elf,
    )?;
    check(
        names
            .iter()
            .any(|name| name.starts_with("BARQ_A_DELIBERATELY_LONG_SYMBOL_NAME")),
        format!("{}: report missing long symbol name", elf.display()),
    )?;
    let hidden = [
        "barq_filter_literal",
        "barq_filter_group_one",
        "barq_filter_group_two",
    ];
    require_names(&names, &hidden, &elf)?;
    let filtered = report(
        barq,
        &elf,
        &[
            "--exclude-symbol",
            "barq_filter_literal",
            "--exclude-symbol-regex",
            "^barq_filter_group_",
        ],
    )?;
    check(
        hidden.iter().all(|name| !filtered.contains(*name)),
        format!(
            "{}: name/regex exclusions did not hide fixture functions",
            elf.display()
        ),
    )?;
    require_names(
        &filtered,
        &["barq_zero_size_marker", "barq_payload_alias"],
        &elf,
    )?;

    let elf = directory.join("large-firmware");
    let large = symbols(&elf)?;
    let functions: Vec<_> = large
        .iter()
        .filter(|(name, symbol)| name.contains("stage_") && symbol.kind == SymbolKind::Text)
        .map(|(name, _)| name.as_str())
        .collect();
    let counters: Vec<_> = large
        .keys()
        .filter(|name| name.contains("stage_") && name.contains("COUNTER"))
        .map(String::as_str)
        .collect();
    check(
        functions.len() == 256 && counters.len() == 256,
        format!(
            "{}: expected 256 functions/counters, found {}/{}",
            elf.display(),
            functions.len(),
            counters.len()
        ),
    )?;
    let names = report(barq, &elf, &[])?;
    require_names(&names, &functions, &elf)?;
    require_names(&names, &counters, &elf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_decodes_symbol_names_without_expanding_entities_twice() {
        let html =
            r#"<span data-name="foo&lt;T&gt;">foo</span><span data-name="a&amp;quot;b"></span>"#;
        assert_eq!(
            report_names(html),
            BTreeSet::from(["foo<T>".to_string(), "a&quot;b".to_string()])
        );
    }
}
