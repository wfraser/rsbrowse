use std::io::Write;
use std::process::Command;

fn main() {
    let git = Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .expect("failed to run 'git rev-parse HEAD'");

    if git.status.success() {
        print!("cargo:rustc-env=GIT_COMMIT_HASH=");
        std::io::stdout().write_all(&git.stdout).unwrap();
    }
}
