use std::fs::{self, File};
use std::iter;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;

pub type Id = rustdoc_types::Id;

/// Write the analysis data to a subdirectory under target/ with this name.
const SUBDIR: &str = "rsbrowse";

pub struct Analysis {
    pub krate: rustdoc_types::Crate,
}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>, compiler: &str) -> anyhow::Result<()> {
        let cargo_status = Command::new("cargo")
            .arg(format!("+{compiler}"))
            .arg("rustdoc")
            .arg("--target-dir")
            .arg(Path::new("target").join(SUBDIR))
            .arg("--")
            .arg("-Zunstable-options")
            .arg("--output-format=json")
            .arg("--document-private-items")
            .arg("--document-hidden-items")
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
        let json_path = fs::read_dir(&root)?
            /*.try_find(|entry| {
                Ok(entry?.file_name().as_encoded_bytes().ends_with(b".json"))
            })?*/
            .find_map(|r| {
                let entry = match r {
                    Ok(v) => v,
                    Err(e) => return Some(Err(e)),
                };
                if entry.file_name().as_encoded_bytes().ends_with(b".json") {
                    Some(Ok(entry))
                } else {
                    None
                }
            })
            .transpose()?
            .ok_or_else(|| anyhow::anyhow!("no json files found"))?
            .path();
        let krate = parse_json(&json_path)
            .with_context(|| json_path.display().to_string())?;
        Ok(Self { krate })
    }

    pub fn crate_ids(&self) -> impl Iterator<Item=CrateId> + '_ {
        let my_name = self.krate.index.iter()
            .find_map(|(_id, item)| {
                match &item.inner {
                    rustdoc_types::ItemEnum::Module(m) if m.is_crate && item.crate_id == 0 => {
                        Some(item.name.clone().expect("crate module should have a name"))
                    }
                    _ => None
                }
            })
            .expect("should have an index item for the local crate");

        let myself = CrateId {
            name: my_name,
            id: 0,
        };

        let others = self.krate.external_crates.iter()
            .map(|(&id, ext)| {
                CrateId {
                    name: ext.name.clone(),
                    id,
                }
            });

        iter::once(myself).chain(others)
    }

    pub fn items<'a>(&'a self, crate_id: &'a CrateId, parent_id: Option<Id>)
        -> impl Iterator<Item = &'a rustdoc_types::Item> + 'a
    {
        let parent_id = parent_id.unwrap_or(self.krate.root.clone());
        let parent = &self.krate.index[&parent_id];

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
            Variant(_) => vec![],
            Function(_) => vec![],
            Trait(t) => {
                // TODO: also include impls?
                t.items.clone()
            }
            TraitAlias(_) => vec![],
            Impl(i) => i.items.clone(),
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

        self.krate.index.iter()
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
    pub id: u32,
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
