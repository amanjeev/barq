use clap::Parser;
use regex::Regex;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "barq",
    about = "Visualize microcontroller Flash and RAM from ELF files"
)]
pub(crate) struct Cli {
    /// Path to the ELF file
    pub(crate) elf: PathBuf,

    /// Output HTML file (default: stdout)
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,

    /// Exclude an exact original ELF name (repeat for multiple names)
    #[arg(long, value_name = "NAME")]
    pub(crate) exclude_symbol: Vec<String>,

    /// Exclude original ELF names matching a regex (repeat for multiple patterns)
    #[arg(long, value_name = "REGEX")]
    pub(crate) exclude_symbol_regex: Vec<Regex>,

    /// Flash memory start address (auto-detected from ELF if not specified)
    #[arg(long)]
    flash_origin: Option<String>,

    /// RAM memory start address (auto-detected from ELF if not specified)
    #[arg(long)]
    ram_origin: Option<String>,
}
