use std::collections::{HashSet, HashMap};
use std::ffi::OsString;
use std::fs::File;
use std::io::BufReader;
use std::os::unix::prelude::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use protobuf::Message;
use scip::types::Occurrence;

#[derive(Debug)]
pub struct Analysis {
    //index: scip::types::Index,
    items: HashMap<String, Item>,
}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>) -> Result<(), String> {
        std::env::set_current_dir(workspace_path.as_ref())
            .map_err(|e| e.to_string())?;

        if Path::new("index.scip").try_exists().unwrap_or(false) {
            return Ok(());
        }

        let mut bin = Command::new("rustup")
            .arg("which")
            .arg("rust-analyzer")
            .output()
            .expect("rustup which rust-analyzer should give a path")
            .stdout;
        if bin.last() == Some(&b'\n') {
            bin.pop();
        }
        let bin = PathBuf::from(OsString::from_vec(bin));
        eprintln!("running {bin:?}");

        Command::new(bin)
            .arg("scip")
            .arg(workspace_path.as_ref())
            .spawn()
            .and_then(|child| child.wait_with_output())
            .map_err(|e| e.to_string())
            .map(|_|())
    }

    pub fn load(workspace_path: impl AsRef<Path>) -> Self {
        let f = File::open(workspace_path.as_ref().join("index.scip"))
            .expect("index.scip should be readable");
        let mut f = BufReader::new(f);
        let mut f = protobuf::CodedInputStream::from_buf_read(&mut f);
        let index = scip::types::Index::parse_from(&mut f)
            .expect("index.scip should be parsed into an index");

        let mut items = HashMap::<String, Item>::new();
        for doc in index.documents {
            for info in doc.symbols {
                let sym = parse_symbol(&info.symbol);
                items.insert(info.symbol.clone(), Item {
                    sym,
                    doc: info.documentation,
                    rel: info.relationships,
                    occur: vec![],
                });
            }

            for occ in doc.occurrences {
                /*if occ.symbol_roles & scip::types::SymbolRole::Definition as i32 == 0 {
                    continue;
                }*/
                if occ.symbol.starts_with("local ") {
                    continue;
                }
                let item = match items.get_mut(&occ.symbol) {
                    Some(item) => item,
                    None => {
                        panic!("no symbol {} found!", occ.symbol);
                    }
                };
                item.occur.push((PathBuf::from(&doc.relative_path), occ));
            }
        }

        //Self { index }
        Self { items }
    }

    pub fn raw_symbols<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        /*self.index.documents
            .iter()
            .map(|doc| {
                doc.symbols.iter()
            })
            .flatten()
            .map(|sym| {
                sym.symbol.as_str()
            })*/
        self.items.keys().map(|s| s.as_str())
    }

    pub fn all_items<'a>(&'a self) -> impl Iterator<Item = &'a Item> + 'a {
        self.items.values()
    }

    pub fn crate_symbols<'a>(&'a self, crate_id: &'a CrateId) -> impl Iterator<Item = scip::types::Symbol> + 'a {
        self.raw_symbols()
            .filter_map(move |s| {
                if s.starts_with(&crate_id.txt_prefix) {
                    let sym = parse_symbol(s);
                    if sym.descriptors.len() == 1 {
                        Some(sym)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
    }

    pub fn symbols_under<'a>(&'a self, parent: &'a scip::types::Symbol) -> impl Iterator<Item = scip::types::Symbol> + 'a {
        let prefix = scip::symbol::format_symbol(parent.clone()); // gratuitous clone >:(
        self.raw_symbols()
            .filter_map(move |s| {
                if let Some(_rest) = s.strip_prefix(&prefix) {
                    let sym = parse_symbol(s);
                    if sym.descriptors.len() == parent.descriptors.len() + 1 {
                        Some(sym)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
    }

    pub fn crate_ids(&self) -> HashSet<CrateId> {
        self.all_items()
            .filter_map(|i| CrateId::from_symbol(i.sym.clone()))
            .collect()
    }

    pub fn specific<'a>(&'a self, sym: &str) -> Option<&'a Item> {
        self.items.get(sym)
    }
}

fn parse_symbol(s: &str) -> scip::types::Symbol {
    match scip::symbol::parse_symbol(s) {
        Ok(sym) => sym,
        Err(e) => {
            panic!("invalid symbol {s:?}: {e:?}");
        }
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CrateId {
    txt_prefix: String,
    pub manager: String,
    pub name: String,
    pub version: String,
}

impl CrateId {
    pub fn from_symbol(mut sym: scip::types::Symbol) -> Option<Self> {
        if sym.scheme == "local" {
            return None;
        }
        sym.descriptors.clear();
        let mut txt_prefix = scip::symbol::format_symbol(sym.clone()); // gratuitous clone required
        txt_prefix.push(' ');
        let pkg = match sym.package.take() {
            Some(s) => s,
            None => {
                panic!("symbol {sym} has no package field");
            }
        };
        Some(CrateId {
            txt_prefix,
            manager: pkg.manager,
            name: pkg.name,
            version: pkg.version,
        })
    }
}

#[derive(Debug)]
pub struct Item {
    pub sym: scip::types::Symbol,
    pub doc: Vec<String>,
    pub rel: Vec<scip::types::Relationship>,
    pub occur: Vec<(PathBuf, Occurrence)>, // (file path, occurrance)
}