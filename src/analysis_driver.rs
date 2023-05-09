use std::path::Path;
use std::process::Command;

pub struct Analysis {}

impl Analysis {
    pub fn generate(workspace_path: impl AsRef<Path>) -> Result<(), String> {
        let me = std::env::current_exe()
            .map_err(|e| format!("failed to get current exe path: {e}"))?;
        let cargo = Command::new("cargo")
            .arg("check")
            .env("RUSTC_WRAPPER", me)
            .current_dir(workspace_path)
            .status()
            .map_err(|e| {
                format!("failed to run 'cargo build': {e}")
            })?;

        if cargo.success() {
            Ok(())
        } else if let Some(code) = cargo.code() {
            Err(format!("'cargo build' failed with exit code {code}"))
        } else {
            Err("'cargo build' killed by signal".to_owned())
        }
    }
}
