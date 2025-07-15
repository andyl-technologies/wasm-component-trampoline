use std::path::PathBuf;

use clap::Parser;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// WASM build artifacts directory
    #[arg(short, long, required = true)]
    pub wasm_dir: PathBuf,

    /// Show verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}
