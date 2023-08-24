use rls_analysis::{AnalysisHost, AnalysisLoader, SearchDirectory};
use std::collections::btree_map::*;
use std::convert::TryFrom;
use std::io::{stderr, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Write the analysis data to a subdirectory under target/ with this name.
const SUBDIR: &str = "rsbrowse";

pub struct Analysis {
    pub crates: Vec<Crate>,
    pub stdlib_crates: Vec<CrateId>,
}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>, compiler: &str) -> Result<(), String> {
        let config_json = serde_json::to_string(
            &rls_data::config::Config {
                output_file: None, // use default paths
                full_docs: true,
                pub_only: false,        // this should be controlled by cmdline args or something
                reachable_only: false,  // this should be controlled by cmdline args or something
                distro_crate: false,
                signatures: false, // this causes rustc to ICE...
                borrow_data: false,
            })
            .expect("failed to json-serialize rust analysis configuration");

        let cargo_status = Command::new("cargo")
            .arg(format!("+{compiler}"))
            .arg("check")
            .arg("--target-dir")
            .arg(Path::new("target").join(SUBDIR))
            .env("RUSTFLAGS", "-Z save-analysis")
            .env("RUST_SAVE_ANALYSIS_CONFIG", &config_json)
            .current_dir(workspace_path)
            .status()
            .map_err(|e|
                format!("failed to run 'cargo build': {e}")
            )?;

        if cargo_status.success() {
            Ok(())
        } else if let Some(code) = cargo_status.code() {
            Err(format!("'cargo build' failed with exit code {code}"))
        } else {
            Err("'cargo build' killed by signal".to_owned())
        }
    }

    pub fn load(workspace_path: impl Into<PathBuf>) -> Self {
        let loader = Loader::new(workspace_path, "debug");
        let crates = rls_analysis::read_analysis_from_files(
                &loader, Default::default(), &[] as &[&str])
            .into_iter()
            .map(|c| Crate::try_from(c).expect("unable to read crate analysis"))
            .collect::<Vec<Crate>>();
        let mut stdlib_crates = vec![];
        if let Some(ref stdlib_base) = loader.stdlib_dir {
            for krate in &crates {
                if krate.inner.path.as_ref().unwrap().starts_with(stdlib_base) {
                    stdlib_crates.push(krate.id());
                }
            }
        }
        Self { crates, stdlib_crates }
    }

    pub fn crate_ids(&self) -> impl Iterator<Item=CrateId> + '_ {
        self.crates.iter()
            .map(|c| c.id())
    }

    pub fn get_crate<'a>(&'a self, id: &CrateId) -> &'a Crate {
        self.try_get_crate(id)
            .unwrap_or_else(|| panic!("no analysis for crate \"{}\"", id.name))
    }

    pub fn try_get_crate<'a>(&'a self, id: &CrateId) -> Option<&'a Crate> {
        self.crates.iter()
            .find(|c| c.matches_id(id))
    }

    pub fn defs<'a>(&'a self, crate_id: &CrateId, parent_id: Option<rls_data::Id>)
        -> impl Iterator<Item=&'a rls_data::Def> + 'a
    {
        let a = &self.get_crate(crate_id).inner.analysis;

        let parent = match parent_id {
            None => {
                a.defs.iter()
                    .find(|def| {
                        def.kind == rls_data::DefKind::Mod
                            && def.name.is_empty()
                    })
                    .expect("missing root module")
            }
            Some(id) => {
                a.defs.iter()
                    .find(|def| def.id == id)
                    .unwrap_or_else(|| panic!("no def found for ID {:?}", id))
            }
        };

        a.defs.iter()
            .filter(move |def| parent.children.contains(&def.id))
    }

    pub fn get_def<'a>(&'a self, crate_id: &CrateId, id: rls_data::Id)
        -> Option<&'a rls_data::Def>
    {
        if id.krate != 0 {
            // External definition. Switch to the defining crate and lookup there.
            // WARNING: the returned def's IDs are all relative to the external crate, not the
            // passed-in crate ID, so use with caution.
            let ext_crate_id = self.get_external_crate_id(crate_id, id)?;
            return self.get_def(
                ext_crate_id,
                rls_data::Id {
                    krate: 0,
                    index: id.index,
                })
        }
        self.try_get_crate(crate_id)?
            .inner
            .analysis
            .defs
            .iter()
            .find(|def| def.id == id)
    }

    pub fn impls<'a>(&'a self, crate_id: &CrateId, parent_id: rls_data::Id)
        -> impl Iterator<Item=ImplDetails> + 'a
    {
        self.get_crate(crate_id)
            .inner
            .analysis
            .relations
            .iter()
            .filter_map(move |rel| match rel.kind {
                rls_data::RelationKind::Impl { id: impl_id } => {
                    if rel.from == parent_id {
                        let trait_id = match rel.to {
                            rls_data::Id { krate: std::u32::MAX, index: std::u32::MAX } => None,
                            other => Some(other),
                        };
                        Some(ImplDetails {
                            impl_id,
                            impl_on: rel.from,
                            trait_id,
                            span: rel.span.clone(),
                        })
                    } else {
                        None
                    }
                }
                rls_data::RelationKind::SuperTrait => None,
            })
    }

    pub fn get_impl<'a>(&'a self, crate_id: &CrateId, impl_id: u32)
        -> Option<&'a rls_data::Impl>
    {
        self.get_crate(crate_id)
            .inner
            .analysis
            .impls
            .iter()
            .find(|i| i.id == impl_id)
    }

    pub fn get_external_crate_id<'a>(&'a self, crate_id: &CrateId, id: rls_data::Id)
        -> Option<&'a CrateId>
    {
        self.get_crate(crate_id)
            .external_crates
            .get(&id.krate)
    }
}

#[derive(Debug, Clone)]
pub struct ImplDetails {
    pub impl_id: u32,
    pub trait_id: Option<rls_data::Id>,
    pub impl_on: rls_data::Id,
    pub span: rls_data::SpanData,
}

#[derive(Debug)]
pub struct Crate {
    inner: rls_analysis::Crate,
    crate_type: CrateType,
    external_crates: BTreeMap<u32, CrateId>,
}

impl Crate {
    pub fn id(&self) -> CrateId {
        CrateId {
            name: self.inner.id.name.clone(),
            crate_type: self.crate_type,
            disambiguator: self.inner.id.disambiguator,
        }
    }

    pub fn matches_id(&self, id: &CrateId) -> bool {
        self.inner.id.name == id.name
            && self.inner.id.disambiguator == id.disambiguator
    }
}

impl std::convert::AsRef<rls_analysis::Crate> for Crate {
    fn as_ref(&self) -> &rls_analysis::Crate {
        &self.inner
    }
}

impl TryFrom<rls_analysis::Crate> for Crate {
    type Error = String;
    fn try_from(inner: rls_analysis::Crate) -> Result<Self, Self::Error> {
        let mut crate_type: Option<CrateType> = None;
        let rustc_args = match inner.analysis.compilation.as_ref() {
            Some(opts) => &opts.arguments,
            None => {
                return Err(format!("missing compilation options in analysis of crate {:?}", inner.id));
            }
        };
        for argpair in rustc_args.windows(2) {
            if argpair[0] == "--crate-type" {
                crate_type = Some(argpair[1].parse::<CrateType>()?);
                break;
            }
        }
        let crate_type = match crate_type {
            Some(val) => val,
            None => {
                return Err(format!("missing crate-type in analysis of crate {:?}", inner.id));
            }
        };
        let external_crates = inner.analysis.prelude.as_ref()
            .map(|p| &p.external_crates[..])
            .unwrap_or(&[])
            .iter()
            .map(|ext| (ext.num, CrateId {
                name: ext.id.name.clone(),
                disambiguator: ext.id.disambiguator,
                crate_type: CrateType::Lib,
            }))
            .collect();
        Ok(Self {
            inner,
            crate_type,
            external_crates,
        })
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct CrateId {
    pub name: String,
    pub crate_type: CrateType,
    pub disambiguator: (u64, u64),
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

#[derive(Clone)]
struct Loader {
    deps_dir: PathBuf,
    stdlib_dir: Option<PathBuf>,
}

impl AnalysisLoader for Loader {
    fn needs_hard_reload(&self, _path_prefix: &Path) -> bool {
        true
    }

    fn fresh_host(&self) -> AnalysisHost<Self> {
        AnalysisHost::new_with_loader(self.clone())
    }

    fn set_path_prefix(&mut self, prefix: &Path) {
        unimplemented!("prefix: {prefix:?}");
    }

    fn abs_path_prefix(&self) -> Option<PathBuf> {
        None
    }

    fn search_directories(&self) -> Vec<SearchDirectory> {
        let mut paths = vec![
            SearchDirectory { path : self.deps_dir.clone(), prefix_rewrite: None }
        ];
        if let Some(path) = self.stdlib_dir.clone() {
            paths.push(SearchDirectory { path, prefix_rewrite: None });
        }
        paths
    }
}

impl Loader {
    pub fn new(path: impl Into<PathBuf>, target: &str) -> Self {
        let deps_dir = path.into()
            .join("target")
            .join(SUBDIR)
            .join(target)
            .join("deps")
            .join("save-analysis");

        Self {
            deps_dir,
            stdlib_dir: get_stdlib_analysis_path(),
        }
    }
}

fn get_stdlib_analysis_path() -> Option<PathBuf> {
    Command::new("rustc")
        .arg("--print")
        .arg("target-libdir")
        .output()
        .map_err(|e| {
            eprintln!("Error running 'rustc --print target-libdir': {e}");
            e
        })
        .ok()
        .and_then(|out| {
            if out.status.success() {
                let path = String::from_utf8(out.stdout)
                    .map_err(|e| {
                        eprintln!("'rustc --print target-libdir' returned invalid utf8: {e}");
                        e
                    })
                    .ok()?;

                Some(PathBuf::from(path.trim_end())
                    .join("..")
                    .join("analysis"))
            } else {
                eprintln!("Error running 'rustc --print target-libdir': {}", out.status);
                eprint!("Command stderr: ");
                stderr().write_all(&out.stderr).unwrap();
                None
            }
        })
}
