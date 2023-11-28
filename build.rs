use std::io::Write;
use std::process::Command;

fn main() {
    if let Ok(git) = Command::new("git").arg("rev-parse").arg("HEAD").output() {
        if git.status.success() {
            print!("cargo:rustc-env=GIT_COMMIT_HASH=");
            std::io::stdout().write_all(&git.stdout).unwrap();
        }
    }
}
