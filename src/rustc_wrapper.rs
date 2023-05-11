use rustc_driver::{Callbacks, Compilation};
use rustc_hir::{AssocItemKind, ItemKind, QPath, Ty, TyKind};
use rustc_hir::def::Res;
use rustc_interface::{Config, Queries};
use rustc_interface::interface::Compiler;
use rustc_span::def_id::{CrateNum, DefId};

use crate::analysis_driver::{Id, Entry, Kind};
use std::fs::File;
use std::io::Write;

struct RsbrowseCallbacks {
    out: File,
}

fn shortid(def_id: DefId) -> Id {
    Id::Def(def_id.krate.as_u32(), def_id.index.as_u32())
}

fn tyid(ty: &Ty<'_>) -> Id {
    if let TyKind::Path(QPath::Resolved(_, p)) = ty.kind {
        if let Some(id) = p.res.opt_def_id() {
            return shortid(id);
        } else if let Res::PrimTy(ty) = p.res {
            return Id::Primitive(ty.name_str().to_owned());
        }
    }
    panic!("type {ty:?} not resolved!");
}

impl RsbrowseCallbacks {
    fn emit_entry(&mut self, e: Entry) {
        serde_json::to_writer(&mut self.out, &e).unwrap();
        self.out.write(b"\n").unwrap();
        self.out.flush().unwrap();
    }

    fn emit(&mut self, id: Id, kind: Kind) {
        self.emit_entry(Entry { id, kind, children: vec![] });
    }

    fn emits(&mut self, id: Id, kind: Kind, children: Vec<Id>) {
        self.emit_entry(Entry { id, kind, children });
    }
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
            self.emit(Id::Crate(0), Kind::Crate(tcx.crate_name(CrateNum::from_u32(0)).as_str().to_owned()));
            for num in tcx.crates(()) {
                // TODO: crate type
                self.emit(Id::Crate(num.as_u32()), Kind::Crate(tcx.crate_name(*num).as_str().to_owned()));
            }
            for id in tcx.hir().items() {
                let hir = tcx.hir();
                let item = hir.item(id);
                let id = shortid(id.owner_id.to_def_id());
                let mut children = vec![];
                let kind = match &item.kind {
                    ItemKind::Use(..) | ItemKind::GlobalAsm(..) | ItemKind::ExternCrate(..) => continue,
                    ItemKind::Static(_ty, ..) => Kind::Static(item.ident.to_string()), // TODO: type?
                    ItemKind::Const(_ty, ..) => Kind::Const(item.ident.to_string()), // TODO: type?
                    ItemKind::Fn(..) => Kind::Fn(item.ident.to_string()),
                    ItemKind::Macro(..) => Kind::Macro(item.ident.to_string()),
                    ItemKind::Mod(m) => {
                        children.extend(m.item_ids
                            .iter()
                            .map(|i| shortid(i.owner_id.to_def_id())));
                        Kind::Mod(item.ident.to_string())
                    }
                    ItemKind::ForeignMod { items, .. } => {
                        children.extend(items
                            .iter()
                            .map(|f| shortid(f.id.owner_id.to_def_id())));
                        Kind::Extern(item.ident.to_string())
                    }
                    //ItemKind::TyAlias(..)
                    //ItemKind::OpaqueTy(..)
                    //ItemKind::Enum(def, generics)
                    ItemKind::Struct(variant_data, _) => {
                        for fd in variant_data.fields() {
                            let id = shortid(fd.def_id.into());
                            self.emit(id.clone(), Kind::Field { name: fd.ident.to_string(), ty: tyid(fd.ty) });
                            children.push(id);
                        }
                        Kind::Struct(item.ident.to_string())
                    }
                    //ItemKind::Union(variant_data, generics)
                    //ItemKind::Trait(is_auto, unsafety, generics, generic_bounds, item_refs)
                    //ItemKind::TraitAlias(generics, generic_bounds)
                    ItemKind::Impl(imp) => {
                        let of = imp.of_trait.as_ref().map(|t| {
                            shortid(t.path.res.def_id())
                        });
                        for item in imp.items {
                            // need to also emit the methods separately b/c they don't appear in
                            // hir.items()
                            let id = shortid(item.id.owner_id.to_def_id());
                            let ctor = match item.kind {
                                AssocItemKind::Const => Kind::Const,
                                AssocItemKind::Fn { .. } => Kind::Fn,
                                AssocItemKind::Type => Kind::Type,
                            };
                            self.emit(id.clone(), ctor(item.ident.to_string()));
                            children.push(id);
                        }
                        Kind::Impl { of, on: tyid(&imp.self_ty) }
                    }
                    _ => {
                        eprintln!("TODO: {item:#?}");
                        continue;
                    },
                };
                self.emits(id, kind, children);
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
    let cwd = std::env::current_dir().unwrap();
    eprintln!("cwd: {:?}", cwd);
    let target = cwd.join("target");
    if let Err(e) = std::fs::create_dir(&target) {
        if e.kind() != std::io::ErrorKind::AlreadyExists {
            panic!("failed to make target dir: {}", e);
        }
    }
    let mut cb = RsbrowseCallbacks {
        out: File::create(target.join("rsbrowse.json")).expect("failed to create rsbrowse.json"),
    };

    let run = rustc_driver::RunCompiler::new(&args, &mut cb);
    if let Err(e) = run.run() {
        eprintln!("Error running rustc: {e:#?}");
    }
}