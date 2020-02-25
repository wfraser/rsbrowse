use std::path::PathBuf;

mod analysis;
use analysis::Analysis;

mod ui;

struct Arguments {
    crate_path: PathBuf,
}

fn usage() {
    eprintln!("usage: {} <crate name> <cargo workspace path>", std::env::args().next().unwrap());
    eprintln!("    or specify '--list-crates' for <crate name> to list all crates in the workspace");
}

fn parse_args() -> Option<Arguments> {
    let crate_path: PathBuf = std::env::args_os()
        .nth(1)?
        .into();
    Some(Arguments {
        crate_path,
    })
}

trait IterExt: Iterator {
    fn single(mut self) -> Result<Self::Item, bool> where Self: Sized {
        match self.next() {
            Some(item) => match self.next() {
                Some(_) => Err(true),
                None => Ok(item),
            }
            None => Err(false),
        }
    }
}

impl<T: ?Sized> IterExt for T where T: Iterator {}

fn crate_name(id: &analysis::CrateId) -> String {
    match id.crate_type {
        analysis::CrateType::Bin => id.name.clone() + " (bin)",
        analysis::CrateType::Lib => id.name.clone(),
        analysis::CrateType::ProcMacro => id.name.clone(),
    }
}

fn main() {
    let args = parse_args()
        .unwrap_or_else(|| {
            usage();
            std::process::exit(1);
        });

    eprintln!("Running Cargo to generate analysis data...");
    Analysis::generate(&args.crate_path).unwrap();

    eprintln!("Reading analysis data...");
    let analysis = Analysis::load(&args.crate_path);

    /*
    if args.crate_name == "--list-crates" {
        for c in analysis.crates() {
            println!("{} ({})",
                crate_name(&c),
                analysis.crate_imported_by(&c)
                    .fold(None::<String>, |result, item| {
                        Some(match result {
                            None => format!("imported by {}", crate_name(&item)),
                            Some(mut s)  => {
                                if !s.is_empty() {
                                    s += ", ";
                                }
                                s += &crate_name(&item);
                                s
                            }
                        })
                    })
                    .unwrap_or_else(|| "root".to_owned()));
        }
        return;
    }
    */

    /*
    let crate_id = analysis.crates()
        .filter(|id| id.name == args.crate_name)
        .single()
        .unwrap_or_else(|found| {
            // TODO: should probably allow specifying the crate disambiguator to avoid this, but
            // this seems like an uncommon case
            eprintln!("{} instances of crates named {:?}",
                if found { "multiple" } else { "no" },
                args.crate_name);
            eprintln!("(this can sometimes be resolved by first doing a 'cargo clean' in the workspace)");
            unimplemented!("support multiple crates with same name");
        });

    println!("top-level definitions:");
    for def in analysis.defs(&crate_id, "").unwrap() {
        println!("\t{:?} {:?} ({}): {}", def.kind, def.name, def.qualname, def.value);
    }
    */

    ui::ui_loop(analysis);
}
