#![forbid(unsafe_code)]

use clap::Parser;

/// Headless Toniator command-line frontend.
#[derive(Debug, Parser)]
#[command(name = "toniator", version, about)]
struct Cli;

fn main() {
    let _ = Cli::parse();
}
