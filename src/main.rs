use std::path::PathBuf;

use rsbrowse::analysis::Analysis;
use rsbrowse::browser_rustdoc::RustdocBrowser;
use rsbrowse::ui;

struct Arguments {
    workspace_path: PathBuf,
    compiler: String,
}

fn usage() {
    eprintln!(
        "usage: {} <cargo workspace path>",
        std::env::args().next().unwrap()
    );
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
    let args = parse_args().unwrap_or_else(|| {
        usage();
        std::process::exit(1);
    });

    eprintln!("Running Cargo to generate analysis data...");
    Analysis::generate(&args.workspace_path, &args.compiler).unwrap();

    eprintln!("Reading analysis data...");
    let analysis = Analysis::load(&args.workspace_path).unwrap();

    std::env::set_current_dir(&args.workspace_path).unwrap();

    let browser = RustdocBrowser::new(analysis);

    //use rsbrowse::browser_trait::Browser;
    //println!("{:#?}", browser.list_crates());

    /*let stuff = browser.list_items(
        &rsbrowse::analysis::CrateId { id: 0, name: String::new() },
        &rsbrowse::browser_rustdoc::Item::Root,
    )
        .into_iter()
        .map(|(label, item)| {
            let id = if let rsbrowse::browser_rustdoc::Item::Item(item) = &item {
                &item.id.0
            } else {
                "root"
            };
            format!("{label} = {id}")
        })
        .collect::<Vec<_>>();
    println!("{:#?}", stuff);*/

    // Mega-hax, but doesn't matter because we're not returning from run() anyway.
    let browser: &'static RustdocBrowser = Box::leak(Box::new(browser));

    ui::run(browser);
}
