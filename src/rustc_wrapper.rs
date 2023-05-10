use rustc_driver::{Callbacks, Compilation};
use rustc_hir::{AssocItemKind, ItemKind, QPath, Ty, TyKind};
use rustc_hir::def::Res;
use rustc_interface::{Config, Queries};
use rustc_interface::interface::Compiler;
use rustc_span::def_id::{CrateNum, DefId};
use rustc_span::symbol::Ident;

struct RsbrowseCallbacks {}

fn shortid(def_id: DefId) -> String {
    format!("{}:{}", def_id.krate.as_u32(), def_id.index.as_u32())
}

fn tyid(ty: &Ty<'_>) -> String {
    if let TyKind::Path(QPath::Resolved(_, p)) = ty.kind {
        if let Some(id) = p.res.opt_def_id() {
            return shortid(id);
        } else if let Res::PrimTy(ty) = p.res {
            return ty.name_str().to_owned();
        }
    }
    panic!("type {ty:?} not resolved!");
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
            eprintln!(r#""0","crate","{}""#, tcx.crate_name(CrateNum::from_u32(0)).as_str());
            for num in tcx.crates(()) {
                // TODO: crate type
                eprintln!(r#""{}","crate","{}""#, num.as_u32(), tcx.crate_name(*num).as_str());
            }
            for id in tcx.hir().items() {
                let hir = tcx.hir();
                let item = hir.item(id);
                let id = shortid(id.owner_id.to_def_id());
                let (kind, name, children) = match &item.kind {
                    ItemKind::Use(..) | ItemKind::GlobalAsm(..) | ItemKind::ExternCrate(..) => continue,
                    ItemKind::Static(_ty, ..) => ("static", item.ident, vec![]), // TODO: type?
                    ItemKind::Const(_ty, ..) => ("const", item.ident, vec![]), // TODO: type?
                    ItemKind::Fn(..) => ("fn", item.ident, vec![]),
                    ItemKind::Macro(..) => ("macro", item.ident, vec![]),
                    ItemKind::Mod(m) => {
                        let children = m.item_ids
                            .iter()
                            .map(|i| shortid(i.owner_id.to_def_id()))
                            .collect::<Vec<_>>();
                        ("mod", item.ident, children)
                    }
                    ItemKind::ForeignMod { items, .. } => {
                        let children = items
                            .iter()
                            .map(|f| shortid(f.id.owner_id.to_def_id()))
                            .collect::<Vec<_>>();
                        ("extern", item.ident, children)
                    }
                    //ItemKind::TyAlias(..)
                    //ItemKind::OpaqueTy(..)
                    //ItemKind::Enum(def, generics)
                    ItemKind::Struct(variant_data, _) => {
                        let mut fields = vec![];
                        for fd in variant_data.fields() {
                            let id = shortid(fd.def_id.into());
                            eprintln!(r#""{id}","field","{}","{}""#, fd.ident, tyid(fd.ty));
                            fields.push(id);
                        }
                        ("struct", item.ident, fields)
                    }
                    //ItemKind::Union(variant_data, generics)
                    //ItemKind::Trait(is_auto, unsafety, generics, generic_bounds, item_refs)
                    //ItemKind::TraitAlias(generics, generic_bounds)
                    ItemKind::Impl(imp) => {
                        let name = if let Some(t) = &imp.of_trait {
                            let t_id = t.path.res.def_id();
                            /*if t_id.is_local() {
                                hir.get_if_local(t_id).unwrap().ident().unwrap()
                            } else {
                                Ident::from_str(&tcx.def_path_str(t_id))
                            }*/
                            //let sym = tcx.item_name(t_id);
                            //Ident::with_dummy_span(sym)
                            tcx.opt_item_ident(t_id).unwrap()
                        } else {
                            Ident::from_str("Self")
                        };
                        let mut children = vec![];
                        // special case: first child is the type the impl is on
                        children.push(tyid(&imp.self_ty));
                        for item in imp.items {
                            // need to also emit the methods separately b/c they don't appear in
                            // hir.items()
                            let id = shortid(item.id.owner_id.to_def_id());
                            let kind = match item.kind {
                                AssocItemKind::Const => "const",
                                AssocItemKind::Fn { .. } => "fn",
                                AssocItemKind::Type => "type",
                            };
                            eprintln!(r#""{id}","{kind}","{}""#, item.ident);
                            children.push(id);
                        }
                        ("impl", name, children)
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