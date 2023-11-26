use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use rayon::prelude::*;

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
            .env(
                "RUSTDOCFLAGS",
                "-Zunstable-options \
                --output-format=json \
                --document-private-items \
                --document-hidden-items \
                ",
            )
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
        let root: PathBuf = workspace_path
            .into()
            .join("target")
            .join(SUBDIR)
            .join("doc");
        let mut paths = vec![];
        for res in fs::read_dir(root)? {
            let entry = res?;
            if entry.file_name().as_encoded_bytes().ends_with(b".json") {
                let path = entry.path();
                let crate_name = path
                    .file_stem()
                    .unwrap()
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("{path:?} isn't utf-8"))?
                    .to_owned();
                paths.push((crate_name, path));
            }
        }

        let crates = paths
            .into_par_iter()
            .map(|(crate_name, path)| {
                println!("reading {path:?}");
                let data = parse_json(&path).with_context(|| path.display().to_string())?;
                Ok((crate_name, data))
            })
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        Ok(Self { crates })
    }

    pub fn crate_ids(&self) -> impl Iterator<Item = CrateId> + '_ {
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

        self.crates
            .values()
            .flat_map(|crate_| &crate_.index)
            .filter_map(|(_id, item)| match &item.inner {
                rustdoc_types::ItemEnum::Module(m) if m.is_crate && item.crate_id == 0 => {
                    let name = item.name.clone().expect("crate module should have a name");
                    Some(CrateId { name })
                }
                _ => None,
            })

        //ids.into_iter()
    }

    pub fn items<'a, 'b>(
        &'a self,
        crate_id: &'b CrateId,
        parent_id: Option<Id>,
    ) -> impl Iterator<Item = Item<'a>> + 'b
    where
        'a: 'b,
    {
        let parent_id = parent_id.unwrap_or(self.crates[&crate_id.name].root.clone());
        let parent = self
            .crates
            .get(&crate_id.name)
            .unwrap_or_else(|| panic!("no crate {crate_id:?}"))
            .index
            .get(&parent_id)
            .unwrap_or_else(|| panic!("no id {parent_id:?} in {crate_id:?}"));

        use rustdoc_types::ItemEnum::*;
        let children = match &parent.inner {
            Module(m) => m.items.clone(),
            ExternCrate { .. } => vec![],
            Import(_) => vec![],
            Union(u) => [&u.fields[..], &u.impls[..]].concat(),
            Struct(s) => {
                let fields = match &s.kind {
                    rustdoc_types::StructKind::Unit => vec![],
                    rustdoc_types::StructKind::Tuple(t) => {
                        t.iter().filter_map(|x| x.as_ref()).cloned().collect()
                    }
                    rustdoc_types::StructKind::Plain { fields, .. } => fields.clone(),
                };
                [&fields[..], &s.impls[..]].concat()
            }
            StructField(ty) => type_ids(ty),
            Enum(e) => [&e.variants[..], &e.impls[..]].concat(),
            Variant(v) => match &v.kind {
                rustdoc_types::VariantKind::Plain => vec![],
                rustdoc_types::VariantKind::Tuple(t) => {
                    t.iter().filter_map(|id| id.clone()).collect()
                }
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
            }
            TypeAlias(ty) => type_ids(&ty.type_),
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

        children.into_iter().filter_map(move |id| {
            if let Some(item) = self.crates[&crate_id.name].index.get(&id) {
                Some(Item::Item(item))
            } else {
                let summary = self.crates[&crate_id.name].paths.get(&id)?;
                let other_crate = &summary.path[0];
                let other_id =
                    self.crates
                        .get(other_crate)?
                        .paths
                        .iter()
                        .find_map(|(id, other)| {
                            if other.path == summary.path {
                                Some(id)
                            } else {
                                None
                            }
                        })?;
                let item = self.crates[other_crate].index.get(other_id)?;
                Some(Item::Foreign(
                    CrateId {
                        name: other_crate.to_owned(),
                    },
                    item,
                ))
            }
        })
    }
}

fn parse_json(p: &Path) -> anyhow::Result<rustdoc_types::Crate> {
    let f = File::open(p)?;
    let data = serde_json::from_reader(BufReader::new(f))?;
    Ok(data)
}

fn type_ids(ty: &rustdoc_types::Type) -> Vec<Id> {
    use rustdoc_types::Type::*;
    match ty {
        ResolvedPath(path) => vec![path.id.clone()],
        DynTrait(dt) => dt.traits.iter().map(|t| t.trait_.id.clone()).collect(),
        Generic(_) => vec![],
        Primitive(_) => vec![],
        FunctionPointer(_) => vec![],
        Tuple(types) => types.iter().map(type_ids).flatten().collect(),
        Slice(ty) => type_ids(ty),
        Array { type_, .. } => type_ids(type_),
        ImplTrait(generics) => generics
            .iter()
            .filter_map(|g| match g {
                rustdoc_types::GenericBound::TraitBound { trait_, .. } => Some(trait_.id.clone()),
                rustdoc_types::GenericBound::Outlives(_) => None,
            })
            .collect(),
        Infer => vec![],
        RawPointer { type_, .. } => type_ids(type_),
        BorrowedRef { type_, .. } => type_ids(type_),
        QualifiedPath {
            self_type, trait_, ..
        } => {
            let from_self = type_ids(&self_type);
            if let Some(t) = trait_ {
                [&from_self[..], &[t.id.clone()]].concat()
            } else {
                from_self
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrateId {
    pub name: String,
    //pub id: u32,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Item<'a> {
    Root,
    Item(&'a rustdoc_types::Item),
    Foreign(CrateId, &'a rustdoc_types::Item),
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
