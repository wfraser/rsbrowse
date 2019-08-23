use rls_analysis::{AnalysisHost, AnalysisLoader, SearchDirectory};
use std::path::{Path, PathBuf};

pub struct Analysis {
    pub crates: Vec<rls_analysis::Crate>,
}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>) -> Result<(), String> {
        let config_json = serde_json::to_string(
            &rls_data::config::Config {
                output_file: None, // use default paths
                full_docs: true,
                pub_only: true,
                reachable_only: true,
                distro_crate: false,
                signatures: false, // this seems to be busted
                borrow_data: false,
            })
            .expect("failed to json-serialize rust analysis configuration");

        let cargo_status = std::process::Command::new("cargo")
            .arg("build")
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
        let crates = rls_analysis::read_analysis_from_files(&loader, Default::default(),
            &[] as &[&str]);
        Self { crates }
    }

    pub fn crates<'a>(&'a self) -> impl Iterator<Item=&'a rls_data::GlobalCrateId> + 'a {
        self.crates.iter().map(|c| &c.id)
    }

    pub fn defs<'a>(&'a self, crate_id: &rls_data::GlobalCrateId, path: &'a str)
        -> Option<impl Iterator<Item=&'a rls_data::Def> + 'a>
    {
        self.crates.iter()
            .find(|c| &c.id == crate_id)
            .map(|c| c.analysis.defs.iter()
                 .filter(move |def| def.parent.is_none() && def.qualname.starts_with(path)))
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
