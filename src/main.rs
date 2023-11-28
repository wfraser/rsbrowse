use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::Context;
use clap::Parser;
use lazy_static::lazy_static;
use log::{info, Log};
use rsbrowse::analysis::Analysis;
use rsbrowse::browser_rustdoc::RustdocBrowser;
use rsbrowse::ui;
use tempfile::NamedTempFile;

#[derive(Debug, Parser)]
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

    *LOGGER.sink.lock().unwrap() = Some(Box::new(io::stderr()));
    log::set_max_level(log::LevelFilter::max());
    let _ = log::set_logger(&*LOGGER);

    eprintln!("Running Cargo to generate analysis data...");
    Analysis::generate(&args.workspace_path, args.toolchain.as_deref())?;

    eprintln!("Reading analysis data...");
    let analysis = Analysis::load(&args.workspace_path)?;

    std::env::set_current_dir(&args.workspace_path)?;

    let browser = RustdocBrowser::new(analysis);

    // Mega-hax, but doesn't matter because we're not returning from run() anyway.
    let browser: &'static RustdocBrowser = Box::leak(Box::new(browser));

    if let Err(e) = log_to_file() {
        eprintln!("failed to set up logging to file: {e}");
        eprintln!("disabling logs");
        *LOGGER.sink.lock().unwrap() = None;
    } else {
        info!("rsbrowse/{}", env!("CARGO_PKG_VERSION"));
        info!(
            "git:{}",
            option_env!("GIT_COMMIT_HASH").unwrap_or("<no git hash>")
        );
        info!("{args:#?}");
        info!("workspace path: {:?}", std::env::current_dir());
    }

    ui::run(browser);
    Ok(())
}

fn log_to_file() -> anyhow::Result<()> {
    let file = NamedTempFile::with_prefix("rsbrowse")?;
    let path = file.path().with_file_name("rsbrowse.log");
    let file = file.persist(&path).context("failed to persist logfile")?;
    *LOGGER.sink.lock().unwrap() = Some(Box::new(file));
    eprintln!("logs written to {path:?}");
    Ok(())
}

lazy_static! {
    static ref LOGGER: Logger = Logger {
        sink: Mutex::new(None)
    };
}

struct Logger {
    sink: Mutex<Option<Box<dyn Write + Send>>>,
}

impl Log for Logger {
    fn enabled(&self, _: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        if let Some(ref mut sink) = *self.sink.lock().unwrap() {
            if let Some(path) = record.module_path() {
                if path.starts_with("cursive") && record.level() <= log::Level::Debug {
                    return;
                }
            }

            let _ = write!(sink, "{}: {}", record.level(), record.args());
            if let Some(path) = record.module_path() {
                let _ = write!(sink, " ({path}");
                if let Some(line) = record.line() {
                    let _ = write!(sink, ":{line}");
                }
                let _ = write!(sink, ")");
            }
            let _ = writeln!(sink);
        }
    }

    fn flush(&self) {}
}
