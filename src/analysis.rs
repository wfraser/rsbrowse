use rls_analysis::{AnalysisHost, AnalysisLoader, SearchDirectory};
use std::convert::TryFrom;
use std::path::{Path, PathBuf};

pub struct Analysis {
    pub crates: Vec<Crate>,
}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>) -> Result<(), String> {
        let config_json = serde_json::to_string(
            &rls_data::config::Config {
                output_file: None, // use default paths
                full_docs: true,
                pub_only: false,        // this should be controlled by cmdline args or something
                reachable_only: false,  // this should be controlled by cmdline args or something
                distro_crate: false,
                signatures: false, // this seems to be busted
                borrow_data: false,
            })
            .expect("failed to json-serialize rust analysis configuration");

        let cargo_status = std::process::Command::new("cargo")
            .arg("check")
            .env("RUSTFLAGS", "-Z save-analysis")
            .env("RUST_SAVE_ANALYSIS_CONFIG", &config_json)
            .current_dir(workspace_path)
            .status()
            .map_err(|e|
                format!("failed to run 'cargo build': {}", e)
            )?;

        if cargo_status.success() {
            Ok(())
        } else if let Some(code) = cargo_status.code() {
            Err(format!("'cargo build' failed with exit code {}", code))
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
            .collect();
        Self { crates }
    }

    pub fn crate_ids<'a>(&'a self) -> impl Iterator<Item=CrateId> + 'a {
        self.crates.iter()
            .map(|c| c.id())
    }

    pub fn get_crate<'a>(&'a self, id: &CrateId) -> &'a Crate {
        self.crates.iter().find(|c| c.matches_id(id)).as_ref().unwrap()
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
                            && def.name == ""
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
        self.get_crate(crate_id)
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
}

#[derive(Debug)]
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
        Ok(Self {
            inner,
            crate_type,
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
}

impl std::str::FromStr for CrateType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "bin" => Self::Bin,
            "lib" => Self::Lib,
            "proc-macro" => Self::ProcMacro,
            "cdylib" => Self::CDylib,
            _ => {
                return Err(format!("unknown crate type {:?}", s));
            }
        })
    }
}

#[derive(Clone)]
struct Loader {
    deps_dir: PathBuf,
}

impl AnalysisLoader for Loader {
    fn needs_hard_reload(&self, _path_prefix: &Path) -> bool {
        true
    }

    fn fresh_host(&self) -> AnalysisHost<Self> {
        AnalysisHost::new_with_loader(self.clone())
    }

    fn set_path_prefix(&mut self, prefix: &Path) {
        unimplemented!("prefix: {:?}", prefix);
    }

    fn abs_path_prefix(&self) -> Option<PathBuf> {
        None
    }

    fn search_directories(&self) -> Vec<SearchDirectory> {
        vec![SearchDirectory { path : self.deps_dir.clone(), prefix_rewrite: None }]
    }
}

impl Loader {
    pub fn new(path: impl Into<PathBuf>, target: &str) -> Self {
        Self {
            deps_dir: path.into()
                .join("target")
                .join(target)
                .join("deps")
                .join("save-analysis"),
        }
    }
}
