use std::path::PathBuf;

mod analysis;
use analysis::Analysis;

mod ui;

struct Arguments {
    workspace_path: PathBuf,
}

fn usage() {
    eprintln!("usage: {} <cargo workspace path>", std::env::args().next().unwrap());
}

fn parse_args() -> Option<Arguments> {
    let workspace_path: PathBuf = std::env::args_os()
        .nth(1)?
        .into();
    Some(Arguments {
        workspace_path,
    })
}

fn main() {
    let args = parse_args()
        .unwrap_or_else(|| {
            usage();
            std::process::exit(1);
        });

    eprintln!("Running Cargo to generate analysis data...");
    Analysis::generate(&args.workspace_path).unwrap();

    eprintln!("Reading analysis data...");
    let analysis = Analysis::load(&args.workspace_path);

    ui::run(analysis);
}
