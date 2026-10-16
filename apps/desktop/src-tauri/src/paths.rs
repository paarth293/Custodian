//! A marked verification profile keeps live acceptance tests away from user state.
use std::path::{Path, PathBuf};
use tauri::Manager;

fn verification_root(path: &Path) -> std::io::Result<PathBuf> {
    let path = path.canonicalize()?;
    if !path.join(".custodian-fixture-root").is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "verification profile requires a fixture marker",
        ));
    }
    Ok(path)
}

fn override_root() -> std::io::Result<Option<PathBuf>> {
    std::env::var_os("CUSTODIAN_VERIFICATION_ROOT")
        .map(|path| verification_root(Path::new(&path)))
        .transpose()
}

/// Live acceptance uses fixture folders as well as isolated application state.
pub fn verification_folder(label: &str) -> std::io::Result<Option<PathBuf>> {
    if ![
        "Downloads",
        "Documents",
        "Desktop",
        "Pictures",
        "Videos",
        "Music",
    ]
    .contains(&label)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "unknown fixture folder",
        ));
    }
    Ok(override_root()?.map(|root| root.join("folders").join(label)))
}

pub fn verification_webview_dir() -> std::io::Result<Option<PathBuf>> {
    Ok(override_root()?.map(|root| root.join("webview")))
}

pub fn data_dir(app: &tauri::AppHandle) -> std::io::Result<PathBuf> {
    if let Some(root) = override_root()? {
        return Ok(root.join("app-data"));
    }
    app.path()
        .app_local_data_dir()
        .map_err(std::io::Error::other)
}

pub fn config_dir(app: &tauri::AppHandle) -> std::io::Result<PathBuf> {
    if let Some(root) = override_root()? {
        return Ok(root.join("config"));
    }
    app.path().app_config_dir().map_err(std::io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_verification_requires_an_explicit_fixture_marker() {
        let dir = tempfile::tempdir().unwrap();
        assert!(verification_root(dir.path()).is_err());
        std::fs::write(dir.path().join(".custodian-fixture-root"), "").unwrap();
        assert_eq!(
            verification_root(dir.path()).unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }
}
