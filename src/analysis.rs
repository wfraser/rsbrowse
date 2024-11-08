use std::backtrace::{Backtrace, BacktraceStatus};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use anyhow::{anyhow, Context};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;

/// Write the analysis data to a subdirectory under target/ with this name.
const SUBDIR: &str = "rsbrowse";

const EMPTY_ID: &rustdoc_types::Id = &rustdoc_types::Id(u32::MAX);
static EMPTY_STRING: String = String::new();

// An ItemId which silently won't resolve to anything.
pub static EMPTY_ITEM_ID: ItemId<'static> = ItemId(
    CrateId {
        name: &EMPTY_STRING,
    },
    EMPTY_ID,
);

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

        let workspace_path = workspace_path.as_ref();

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

        if let Err(e) = copy_stdlib_json(workspace_path, toolchain) {
            error!("Error copying stdlib analysis: {e}");
            error!("Standard library types will not be inspectable.");
        }

        Ok(())
    }

    pub fn load(workspace_path: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let root = json_root(&workspace_path.into());
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

        let progress = ProgressBar::new(paths.len() as u64)
            .with_style(
                ProgressStyle::with_template(
                    "{prefix:>12.cyan.bold} [{bar:25}]: {pos}/{len} {wide_msg}",
                )?
                .progress_chars("=> "),
            )
            .with_prefix("Loading");
        let active = Mutex::new(vec![]);

        let crates = paths
            .into_par_iter()
            .map(|(crate_name, path)| {
                let filename = path.file_stem().unwrap().to_string_lossy().into_owned();
                {
                    let mut active = active.lock().unwrap();
                    active.push(filename.clone());
                    progress.println(format!("reading {path:?}"));
                    progress.set_message(active.join(", "));
                }
                let data = parse_json(&path).with_context(|| path.display().to_string())?;
                {
                    let mut active = active.lock().unwrap();
                    active.retain(|f| f != &filename);
                    progress.set_message(active.join(", "));
                    progress.inc(1);
                }
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
        // Look up the parent item, including possibly resolving it to an item in a different crate
        // (i.e. parent_id may change).
        let (parent_id, parent) = if parent_id == &EMPTY_ITEM_ID {
            (parent_id.clone(), None)
        } else {
            match self.get_item(parent_id.clone()) {
                Some((resolved_id, item)) => match item {
                    Item::Item(i) => (resolved_id, Some(i)),
                    Item::Root => panic!("unexpected Item::Root from get_item()"),
                },
                None => (parent_id.clone(), None),
            }
        };

        // Collect (crate-local) IDs of children depending on the kind of parent it is.
        let children: Vec<&'a rustdoc_types::Id> = if let Some(parent) = parent {
            use rustdoc_types::ItemEnum::*;
            match &parent.inner {
                _ if parent_id == EMPTY_ITEM_ID => vec![],
                Module(m) => m.items.iter().collect(),
                ExternCrate { .. } => vec![],
                Use(_) => vec![],
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
                Constant { type_, .. } => type_ids(type_),
                Static(_) => vec![],
                ExternType => vec![],
                Macro(_) => vec![],
                ProcMacro(_) => vec![],
                Primitive(_) => vec![],
                AssocConst { .. } => vec![],
                AssocType { .. } => vec![],
            }
        } else {
            vec![]
        };

        // Look up and return all the children. The lookup may follow references into other crates.
        children
            .into_iter()
            .filter_map(move |id| self.get_item(parent_id.crate_sibling(id)))
    }

    pub fn get_item<'a>(&'a self, id: ItemId<'a>) -> Option<(ItemId<'a>, Item<'a>)> {
        if id == EMPTY_ITEM_ID {
            return None;
        }
        let ItemId(local_crate_id, mut local_id) = &id;
        let local_crate = self.crates.get(local_crate_id.name)?;
        if local_id == EMPTY_ID {
            // Fake ID of the crate root. Look up what the root actually is.
            local_id = &local_crate.root;
        }
        if let Some(item) = local_crate.index.get(local_id) {
            Some((id, Item::Item(item)))
        } else {
            // Wasn't found in the local crate's index; look up the summary in paths.
            let summary = local_crate.paths.get(local_id)?;
            let other_crate = &summary.path[0];
            // Try looking up by path in the other crate's analysis (if we have it).
            let other_id = self
                .crates
                .get(other_crate)
                .or_else(|| {
                    warn!(
                        "no analysis found for crate {other_crate} (looking for {})",
                        summary.path.join("::")
                    );
                    None
                })?
                .paths
                .iter()
                .find_map(|(id, other)| {
                    if other.path == summary.path {
                        Some(id)
                    } else {
                        None
                    }
                })
                .or_else(|| {
                    error!("no item found for {}", summary.path.join("::"));
                    let bt = Backtrace::capture();
                    if bt.status() == BacktraceStatus::Captured {
                        error!("{bt}");
                    }
                    None
                })?;
            let item = self.crates[other_crate].index.get(other_id)?;
            Some((
                ItemId(CrateId { name: other_crate }, other_id),
                Item::Item(item),
            ))
        }
    }

    pub fn get_path<'a>(&'a self, id: ItemId<'a>, name_hint: &str) -> Option<&'a [String]> {
        if id == EMPTY_ITEM_ID {
            return None;
        }
        Some(
            &self.crates[id.0.name]
                .paths
                .get(id.1)
                .or_else(|| {
                    warn!("missing path for {id:?} ({name_hint})");
                    None
                })?
                .path[..],
        )
    }
}

fn parse_json(p: &Path) -> anyhow::Result<rustdoc_types::Crate> {
    let f = File::open(p)?;
    let data = serde_json::from_reader(BufReader::new(f))?;
    Ok(data)
}

pub fn type_ids(ty: &rustdoc_types::Type) -> Vec<&rustdoc_types::Id> {
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
                rustdoc_types::GenericBound::Use(_) => None,
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
        Pat { type_, .. } => type_ids(type_),
    }
}

fn json_root(workspace_path: &Path) -> PathBuf {
    workspace_path.join("target").join(SUBDIR).join("doc")
}

pub fn get_stdlib_analysis_path(toolchain: Option<&str>) -> anyhow::Result<PathBuf> {
    let mut cmd = Command::new("rustc");
    if let Some(toolchain) = toolchain {
        cmd.arg(format!("+{toolchain}"));
    }

    let out = cmd
        .arg("--print")
        .arg("sysroot")
        .output()
        .context("Error running 'rustc --print sysroot'")?;

    if !out.status.success() {
        let mut msg = format!("Error running 'rustc --print sysroot': {}", out.status).into_bytes();
        msg.extend(b"\nCommand stderr: ");
        msg.extend(&out.stderr);
        anyhow::bail!(String::from_utf8_lossy(&msg).into_owned());
    }

    let sysroot_path = String::from_utf8(out.stdout).context("'rustc --print sysroot' output")?;

    let path = PathBuf::from(sysroot_path.trim())
        .join("share")
        .join("doc")
        .join("rust")
        .join("json");

    if fs::metadata(&path)?.is_dir() {
        Ok(path)
    } else {
        Err(anyhow!("{path:?} is something other than a directory"))
    }
}

fn copy_stdlib_json(workspace_path: &Path, toolchain: Option<&str>) -> anyhow::Result<()> {
    let src = get_stdlib_analysis_path(toolchain)?;
    let dst = json_root(workspace_path);
    for res in fs::read_dir(&src).with_context(|| src.display().to_string())? {
        let entry = res?;
        if entry.file_name().as_encoded_bytes().ends_with(b".json") {
            let src_path = entry.path();
            println!("copying {:?}", entry.file_name());
            fs::copy(&src_path, dst.join(entry.file_name()))
                .with_context(|| format!("copy {src_path:?}"))?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq)]
pub struct CrateId<'a> {
    pub name: &'a String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ItemId<'a>(CrateId<'a>, &'a rustdoc_types::Id);

impl<'a> ItemId<'a> {
    pub fn crate_root(crate_id: CrateId<'a>) -> Self {
        Self(crate_id, EMPTY_ID)
    }

    pub fn crate_name(&self) -> &str {
        self.0.name
    }

    pub fn crate_sibling(&self, other_id: &'a rustdoc_types::Id) -> Self {
        Self(CrateId { name: self.0.name }, other_id)
    }
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum Item<'a> {
    Root,
    Item(&'a rustdoc_types::Item),
}
