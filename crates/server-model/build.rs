use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = match manifest_dir.parent().and_then(|path| path.parent()) {
        Some(root) => root.to_path_buf(),
        None => return,
    };

    let git_dir = repo_root.join(".git");
    if !git_dir.exists() {
        return;
    }

    let hooks_dir = git_dir.join("hooks");
    if let Err(err) = fs::create_dir_all(&hooks_dir) {
        println!("cargo:warning=Failed to create git hooks directory: {err}");
        return;
    }

    for (name, relative) in [
        ("pre-commit", "ci/git_hooks/pre-commit"),
        ("pre-push", "ci/git_hooks/pre-push"),
    ] {
        let source = repo_root.join(relative);
        println!("cargo:rerun-if-changed={}", source.display());
        if !source.exists() {
            println!("cargo:warning=Git hook template missing: {}", source.display());
            continue;
        }

        let destination = hooks_dir.join(name);
        if let Err(err) = fs::copy(&source, &destination) {
            println!("cargo:warning=Failed to install {name} hook: {err}");
            continue;
        }

        #[cfg(unix)]
        if let Err(err) = set_executable(&destination) {
            println!("cargo:warning=Failed to set executable bit on {name} hook: {err}");
        }
    }
}

#[cfg(unix)]
fn set_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_executable(_: &Path) -> std::io::Result<()> {
    Ok(())
}
