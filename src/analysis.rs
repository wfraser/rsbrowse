use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use rayon::prelude::*;

/// Write the analysis data to a subdirectory under target/ with this name.
const SUBDIR: &str = "rsbrowse";

const EMPTY_ID: &rustdoc_types::Id = &rustdoc_types::Id(String::new());

pub struct Analysis {
    pub crates: HashMap<String, rustdoc_types::Crate>,
}

impl Analysis {
    pub fn generate(
        workspace_path: impl AsRef<Path>,
        toolchain: Option<&str>,
    ) -> anyhow::Result<()> {
        let mut cmd = Command::new("cargo");
        if let Some(toolchain) = toolchain {
            cmd.arg(format!("+{toolchain}"));
        }

        let cargo_status = cmd
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

    pub fn crate_ids(&self) -> impl Iterator<Item = ItemId> + '_ {
        self.crates
            .values()
            .flat_map(|crate_| &crate_.index)
            .filter_map(|(_id, item)| match &item.inner {
                rustdoc_types::ItemEnum::Module(m) if m.is_crate && item.crate_id == 0 => {
                    let name = item.name.as_ref().expect("crate module should have a name");
                    Some(ItemId::crate_root(CrateId { name }))
                }
                _ => None,
            })
    }

    pub fn items<'a, 'b>(
        &'a self,
        parent_id: &'b ItemId<'a>,
    ) -> impl Iterator<Item = (ItemId<'a>, Item<'a>)> + 'b
    where
        'a: 'b,
    {
        let ItemId(parent_crate, parent_id) = parent_id;
        let parent = {
            let crate_ = &self
                .crates
                .get(parent_crate.name)
                .unwrap_or_else(|| panic!("no crate {parent_crate:?}"));
            let id = if parent_id == &EMPTY_ID {
                &crate_.root
            } else {
                parent_id
            };
            crate_
                .index
                .get(id)
                .unwrap_or_else(|| panic!("no id {id:?} in {parent_crate:?}"))
        };

        use rustdoc_types::ItemEnum::*;
        let children: Vec<&'a rustdoc_types::Id> = match &parent.inner {
            Module(m) => m.items.iter().collect(),
            ExternCrate { .. } => vec![],
            Import(_) => vec![],
            Union(u) => u.fields.iter().chain(&u.impls).collect(),
            Struct(s) => {
                let fields = match &s.kind {
                    rustdoc_types::StructKind::Unit => vec![],
                    rustdoc_types::StructKind::Tuple(t) => {
                        t.iter().filter_map(|x| x.as_ref()).collect()
                    }
                    rustdoc_types::StructKind::Plain { fields, .. } => fields.iter().collect(),
                };
                fields.into_iter().chain(&s.impls).collect()
            }
            StructField(ty) => type_ids(ty),
            Enum(e) => e.variants.iter().chain(&e.impls).collect(),
            Variant(v) => match &v.kind {
                rustdoc_types::VariantKind::Plain => vec![],
                rustdoc_types::VariantKind::Tuple(t) => {
                    t.iter().filter_map(|id| id.as_ref()).collect()
                }
                rustdoc_types::VariantKind::Struct { fields, .. } => fields.iter().collect(),
            },
            Function(_) => vec![],
            Trait(t) => {
                // TODO: also find impls?
                t.items.iter().collect()
            }
            TraitAlias(_) => vec![],
            Impl(i) => {
                i.items
                    .iter()
                    // Add a reference to the trait itself too if it's not an inherent impl:
                    .chain(i.trait_.as_ref().map(|t| &t.id))
                    .collect()
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
            if let Some(item) = self.crates[parent_crate.name].index.get(id) {
                Some((
                    ItemId(
                        CrateId {
                            name: parent_crate.name,
                        },
                        id,
                    ),
                    Item::Item(item),
                ))
            } else {
                let summary = self.crates[parent_crate.name].paths.get(id)?;
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
                Some((
                    ItemId(CrateId { name: other_crate }, other_id),
                    Item::Item(item),
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

fn type_ids(ty: &rustdoc_types::Type) -> Vec<&rustdoc_types::Id> {
    use rustdoc_types::Type::*;
    match ty {
        ResolvedPath(path) => vec![&path.id],
        DynTrait(dt) => dt.traits.iter().map(|t| &t.trait_.id).collect(),
        Generic(_) => vec![],
        Primitive(_) => vec![],
        FunctionPointer(_) => vec![],
        Tuple(types) => types.iter().flat_map(type_ids).collect(),
        Slice(ty) => type_ids(ty),
        Array { type_, .. } => type_ids(type_),
        ImplTrait(generics) => generics
            .iter()
            .filter_map(|g| match g {
                rustdoc_types::GenericBound::TraitBound { trait_, .. } => Some(&trait_.id),
                rustdoc_types::GenericBound::Outlives(_) => None,
            })
            .collect(),
        Infer => vec![],
        RawPointer { type_, .. } => type_ids(type_),
        BorrowedRef { type_, .. } => type_ids(type_),
        QualifiedPath {
            self_type, trait_, ..
        } => {
            let from_self = type_ids(self_type);
            if let Some(t) = trait_ {
                [&from_self[..], &[&t.id]].concat()
            } else {
                from_self
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CrateId<'a> {
    pub name: &'a String,
}

#[derive(Debug, Clone)]
pub struct ItemId<'a>(CrateId<'a>, &'a rustdoc_types::Id);

impl<'a> ItemId<'a> {
    pub fn crate_root(crate_id: CrateId<'a>) -> Self {
        Self(crate_id, EMPTY_ID)
    }

    pub fn crate_name(&self) -> &str {
        self.0.name
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Item<'a> {
    Root,
    Item(&'a rustdoc_types::Item),
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
