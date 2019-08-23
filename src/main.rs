use std::path::PathBuf;

mod analysis;
use analysis::Analysis;

struct Arguments {
    crate_path: PathBuf,
    crate_name: String,
}

fn usage() {
    eprintln!("usage: {} <crate name> <cargo workspace path>", std::env::args().next().unwrap());
}

fn parse_args() -> Option<Arguments> {
    let crate_name = std::env::args()
        .nth(1)?;
    let crate_path: PathBuf = std::env::args_os()
        .nth(2)?
        .into();
    Some(Arguments {
        crate_name,
        crate_path,
    })
}

fn main() {
    let args = parse_args()
        .unwrap_or_else(|| {
            usage();
            std::process::exit(1);
        });

    eprintln!("Building crate to generate analysis data...");
    Analysis::generate(&args.crate_path).unwrap();

    eprintln!("Reading analysis data...");
    let analysis = Analysis::load(&args.crate_path);
    for c in analysis.crates {
        if c.id.name == args.crate_name {
            println!("Top-level definitions:");
            for def in c.analysis.defs {
                if def.parent.is_none() {
                    println!("\t{:?} {:?} ({}): {}", def.kind, def.name, def.qualname, def.value);
                }
            }
        }
    }
}
