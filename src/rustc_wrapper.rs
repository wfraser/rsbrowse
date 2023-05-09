use rustc_driver::{Callbacks, Compilation};
use rustc_hir::ItemKind;
use rustc_interface::{Config, Queries};
use rustc_interface::interface::Compiler;
use rustc_span::def_id::DefId;

struct RsbrowseCallbacks {}

fn shortid(def_id: DefId) -> String {
    format!("{}:{}", def_id.krate.as_u32(), def_id.index.as_u32())
}

impl Callbacks for RsbrowseCallbacks {
    fn config(&mut self, config: &mut Config) {
        eprintln!("input path: {:?}", config.input.source_name());
        eprintln!("output dir: {:?}", config.output_dir);
        eprintln!("output file: {:?}", config.output_file);
    }

    fn after_analysis<'tcx>(&mut self, compiler: &Compiler, queries: &'tcx Queries<'tcx>) -> Compilation {
        eprintln!("crate name (1): {}", rustc_session::output::find_crate_name(compiler.session(), &[]));

        queries.global_ctxt().expect("global_ctxt").enter(|tcx| {
            let crate_name = tcx.crate_name(rustc_span::def_id::CrateNum::from_u32(0));
            eprintln!("crate name (2): {}", crate_name.as_str());
            for id in tcx.hir().items() {
                let hir = tcx.hir();
                let item = hir.item(id);
                let id = shortid(id.owner_id.to_def_id());
                let (kind, name, children) = match &item.kind {
                    ItemKind::Use(..) | ItemKind::GlobalAsm(..) | ItemKind::ExternCrate(..) => continue,
                    ItemKind::Static(ty, ..) => ("static", item.ident, vec![]),
                    ItemKind::Const(ty, ..) => ("const", item.ident, vec![]),
                    ItemKind::Fn(..) => ("fn", item.ident, vec![]),
                    ItemKind::Macro(..) => ("macro", item.ident, vec![]),
                    ItemKind::Mod(m) => {
                        let children = m.item_ids
                            .iter()
                            .map(|i| shortid(i.owner_id.to_def_id()))
                            .collect::<Vec<_>>();
                        ("mod", item.ident, children)
                    }
                    _ => {
                        eprintln!("TODO: {item:#?}");
                        continue;
                    },
                };
                eprint!(r#""{id}","{kind}","{name}""#);
                for c in children {
                    eprint!(",\"{c}\"");
                }
                eprintln!();
            }
        });

        Compilation::Continue
    }
}

pub fn run() {
    let mut args = std::env::args().collect::<Vec<_>>();

    if args.get(1).map(|s| s.as_str()) == Some("rustc") {
        args.remove(1);
    }

    eprintln!("args: {args:?}");
    let mut cb = RsbrowseCallbacks {};

    let run = rustc_driver::RunCompiler::new(&args, &mut cb);
    if let Err(e) = run.run() {
        eprintln!("Error running rustc: {e:#?}");
    }
}