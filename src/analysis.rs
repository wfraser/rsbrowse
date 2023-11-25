use std::collections::HashMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

pub type Id = rustdoc_types::Id;

/// Write the analysis data to a subdirectory under target/ with this name.
const SUBDIR: &str = "rsbrowse";

pub struct Analysis {
    pub crates: HashMap<String, rustdoc_types::Crate>,
}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>, compiler: &str) -> anyhow::Result<()> {
        let cargo_status = Command::new("cargo")
            .arg(format!("+{compiler}"))
            .arg("doc")
            .arg("--target-dir")
            .arg(Path::new("target").join(SUBDIR))
            .arg("--workspace")
            .env("RUSTDOCFLAGS", "-Zunstable-options \
                --output-format=json \
                --document-private-items \
                --document-hidden-items \
                ")
            .current_dir(workspace_path)
            .status()
            .context("failed to run 'cargo rustdoc'")?;

        if !cargo_status.success() {
            if let Some(code) = cargo_status.code() {
                anyhow::bail!("'cargo build' failed with exit code {code}");
            } else {
                anyhow::bail!("'cargo build' killed by signal");
            }
        }
        Ok(())
    }

    pub fn load(workspace_path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let root: PathBuf = workspace_path.into().join("target").join(SUBDIR).join("doc");
        let mut crates = HashMap::new();
        for res in fs::read_dir(root)? {
            let entry = res?;
            if entry.file_name().as_encoded_bytes().ends_with(b".json") {
                let path = entry.path();
                let crate_name = path.file_stem().unwrap().to_str()
                    .ok_or_else(|| anyhow::anyhow!("{path:?} isn't utf-8"))?
                    .to_owned();
                println!("reading {path:?}");
                let data = parse_json(&path)
                    .with_context(|| path.display().to_string())?;
                crates.insert(crate_name, data);
            }
        }
        Ok(Self { crates })
    }

    pub fn crate_ids(&self) -> impl Iterator<Item=CrateId> + '_ {
        /*let mut ids = vec![];

        for c in self.crates.values() {
            let name = c.index.iter()
                .find_map(|(_id, item)| {
                    match &item.inner {
                        rustdoc_types::ItemEnum::Module(m) if m.is_crate && item.crate_id == 0 => {
                            Some(item.name.clone().expect("crate module should have a name"))
                        }
                        _ => None
                    }
                })
                .expect("should have an index item for the local crate");

            ids.push(CrateId { name });
        }*/

        self.crates.values()
            .flat_map(|crate_| &crate_.index)
            .filter_map(|(_id, item)| {
                match &item.inner {
                    rustdoc_types::ItemEnum::Module(m) if m.is_crate && item.crate_id == 0 => {
                        let name = item.name.clone().expect("crate module should have a name");
                        Some(CrateId { name })
                    }
                    _ => None
                }
            })

        //ids.into_iter()
    }

    pub fn items<'a>(&'a self, crate_id: &'a CrateId, parent_id: Option<Id>)
        -> impl Iterator<Item = &'a rustdoc_types::Item> + 'a
    {
        let parent_id = parent_id.unwrap_or(self.crates[&crate_id.name].root.clone());
        let parent = &self.crates[&crate_id.name].index[&parent_id];

        use rustdoc_types::ItemEnum::*;
        let children = match &parent.inner {
            Module(m) => m.items.clone(),
            ExternCrate { .. } => vec![],
            Import(_) => vec![],
            Union(u) => [&u.fields[..], &u.impls[..]].concat(),
            Struct(s) => {
                let fields = match &s.kind {
                    rustdoc_types::StructKind::Unit => vec![],
                    rustdoc_types::StructKind::Tuple(t) => t.iter().filter_map(|x| x.as_ref()).cloned().collect(),
                    rustdoc_types::StructKind::Plain { fields, .. } => fields.clone(),
                };
                [&fields[..], &s.impls[..]].concat()
            }
            StructField(_) => vec![],
            Enum(e) => [&e.variants[..], &e.impls[..]].concat(),
            Variant(v) => match &v.kind {
                rustdoc_types::VariantKind::Plain => vec![],
                rustdoc_types::VariantKind::Tuple(t) => t.iter().filter_map(|id| id.clone()).collect(),
                rustdoc_types::VariantKind::Struct { fields, .. } => fields.clone(),
            },
            Function(_) => vec![],
            Trait(t) => {
                // TODO: also find impls?
                t.items.clone()
            }
            TraitAlias(_) => vec![],
            Impl(i) => {
                let mut items = i.items.clone();
                // Add a reference to the trait itself too if it's not an inherent impl:
                if let Some(trait_) = &i.trait_ {
                    items.push(trait_.id.clone());
                }
                items
            },
            TypeAlias(_) => vec![],
            OpaqueTy(_) => vec![],
            Constant(_) => vec![],
            Static(_) => vec![],
            ForeignType => vec![],
            Macro(_) => vec![],
            ProcMacro(_) => vec![],
            Primitive(_) => vec![],
            AssocConst { .. } => vec![],
            AssocType { .. } => vec![],
        };

        self.crates[&crate_id.name].index.iter()
            .filter_map(move |(id, item)| {
                if children.contains(id) {
                    Some(item)
                } else {
                    None
                }
            })
    }
}

fn parse_json(p: &Path) -> anyhow::Result<rustdoc_types::Crate> {
    let f = File::open(p)?;
    let krate = serde_json::from_reader(f)?;
    Ok(krate)
}

#[derive(Debug, Clone)]
pub struct CrateId {
    pub name: String,
    //pub id: u32,
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq)]
pub enum CrateType {
    Bin,
    Lib,
    ProcMacro,
    CDylib,
    Dylib,
}

impl std::str::FromStr for CrateType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "bin" => Self::Bin,
            "lib" => Self::Lib,
            "proc-macro" => Self::ProcMacro,
            "cdylib" => Self::CDylib,
            "dylib" => Self::Dylib,
            _ => {
                return Err(format!("unknown crate type {s:?}"));
            }
        })
    }
}
