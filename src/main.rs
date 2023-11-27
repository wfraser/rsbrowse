use std::path::PathBuf;

use clap::Parser;
use rsbrowse::analysis::Analysis;
use rsbrowse::browser_rustdoc::RustdocBrowser;
use rsbrowse::ui;

#[derive(Parser)]
struct Arguments {
    /// Cargo workspace path
    #[arg()]
    workspace_path: PathBuf,

    /// Select rust toolchain to use.
    /// To disable this flag (i.e. if you don't use rustup), set it to empty string.
    #[arg(long, default_value = "nightly")]
    toolchain: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let mut args = Arguments::parse();
    if args.toolchain.as_deref() == Some("") {
        args.toolchain = None;
    }

    eprintln!("Running Cargo to generate analysis data...");
    Analysis::generate(&args.workspace_path, args.toolchain.as_deref())?;

    eprintln!("Reading analysis data...");
    let analysis = Analysis::load(&args.workspace_path)?;

    std::env::set_current_dir(&args.workspace_path)?;

    let browser = RustdocBrowser::new(analysis);

    // Mega-hax, but doesn't matter because we're not returning from run() anyway.
    let browser: &'static RustdocBrowser = Box::leak(Box::new(browser));

    ui::run(browser);
    Ok(())
}
