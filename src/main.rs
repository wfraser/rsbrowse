use std::path::PathBuf;

use rsbrowse::analysis::Analysis;
use rsbrowse::browser_rls::RlsBrowser;
use rsbrowse::ui;

struct Arguments {
    workspace_path: PathBuf,
    compiler: String,
}

fn usage() {
    eprintln!("usage: {} <cargo workspace path>", std::env::args().next().unwrap());
}

fn parse_args() -> Option<Arguments> {
    let mut args = std::env::args_os().skip(1);
    let mut path = None;
    let mut compiler = "nightly".to_owned();
    loop {
        let arg = match args.next() {
            Some(a) => a,
            None => break,
        };
        match arg.to_str() {
            Some("--compiler") => {
                compiler = args.next()?.to_string_lossy().into_owned();
            }
            _ => {
                if path.is_some() {
                    return None;
                }
                path = Some(arg);
            }
        }
    }
    Some(Arguments {
        workspace_path: path?.into(),
        compiler,
    })
}

fn main() {
    let args = parse_args()
        .unwrap_or_else(|| {
            usage();
            std::process::exit(1);
        });

    eprintln!("Running Cargo to generate analysis data...");
    Analysis::generate(&args.workspace_path, &args.compiler).unwrap();

    eprintln!("Reading analysis data...");
    let analysis = Analysis::load(&args.workspace_path);

    std::env::set_current_dir(&args.workspace_path).unwrap();

    let browser = RlsBrowser::new(analysis);
    ui::run(browser);
}
