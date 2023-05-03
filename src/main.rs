use clap::Parser;
use std::fmt::Display;
use std::path::PathBuf;

use rsbrowse::ui;

#[derive(Debug, Parser)]
struct Arguments {
    /// Path to the Cargo workspace root.
    workspace_path: PathBuf,

    /// What analysis engine to use.
    #[arg(long, default_value_t = AnalysisMode::Rls)]
    mode: AnalysisMode,
}

#[derive(Debug, Clone, Copy)]
enum AnalysisMode {
    /// Use the `-Z save-analysis` feature of rustc, part of RLS. Only available in rust v0.70 and below.
    Rls,

    /// Use the SCIP output of rust-analyzer.
    Scip,
}

impl AnalysisMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Rls => "rls",
            Self::Scip => "scip",
        }
    }
}

impl clap::ValueEnum for AnalysisMode {
    fn value_variants<'a>() -> &'a [Self] {
        &[Self::Rls, Self::Scip]
    }

    fn to_possible_value(&self) -> Option<clap::builder::PossibleValue> {
        Some(clap::builder::PossibleValue::new(self.as_str()))
    }
}

impl Display for AnalysisMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

fn main() {
    let args = Arguments::parse();

    match args.mode {
        AnalysisMode::Rls => {
            use rsbrowse::analysis_rls::Analysis;
            use rsbrowse::browser_rls::RlsBrowser;

            eprintln!("Running Cargo to generate analysis data...");
            Analysis::generate(&args.workspace_path).unwrap();
            eprintln!("Reading analysis data...");
            let analysis = Analysis::load(&args.workspace_path);

            std::env::set_current_dir(&args.workspace_path).unwrap();
            let browser = RlsBrowser::new(analysis);
            ui::run(browser);
        }
        AnalysisMode::Scip => {
            use rsbrowse::analysis_scip::Analysis;
            use rsbrowse::browser_scip::ScipBrowser;

            eprintln!("Running rust-analyzer to generate analysis data...");
            Analysis::generate(&args.workspace_path).unwrap();
            eprintln!("Reading analysis data...");
            let analysis = Analysis::load(&args.workspace_path);
            let browser = ScipBrowser::new(analysis);
            ui::run(browser);
        }
    }
}
