use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
const LIBRARY_NAMES: &[&str] = &["onnxruntime.dll"];
#[cfg(target_os = "macos")]
const LIBRARY_NAMES: &[&str] = &["libonnxruntime.dylib"];
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
const LIBRARY_NAMES: &[&str] = &["libonnxruntime.so"];

pub fn find_in(dir: &Path) -> Option<PathBuf> {
    LIBRARY_NAMES
        .iter()
        .map(|name| dir.join(name))
        .find(|path| path.is_file())
}

/// Points `ORT_DYLIB_PATH` at the runtime shipped next to the executable unless the user set it.
pub fn configure() {
    if std::env::var_os("ORT_DYLIB_PATH").is_some_and(|v| !v.is_empty()) {
        return;
    }
    let Some(found) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
        .and_then(|dir| find_in(&dir))
    else {
        return;
    };
    std::env::set_var("ORT_DYLIB_PATH", found);
}

/// A missing library makes `ort` hang inside its own initialisation, so check that it loads first.
pub fn verify() -> anyhow::Result<()> {
    let target = std::env::var_os("ORT_DYLIB_PATH")
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| LIBRARY_NAMES[0].into());
    // SAFETY: loading the ONNX Runtime runs its initialisers, the same as `ort` does when it loads it.
    unsafe { libloading::Library::new(&target) }
        .map(drop)
        .map_err(|e| {
            anyhow::anyhow!(
                "the ONNX Runtime library could not be loaded ({e}); keep the library that came \
                 with the release next to the bible-mcp executable, or set ORT_DYLIB_PATH to its location"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_library_beside_executable() {
        let dir = tempfile::tempdir().unwrap();
        assert!(find_in(dir.path()).is_none());
        let lib = dir.path().join(LIBRARY_NAMES[0]);
        std::fs::write(&lib, b"").unwrap();
        assert_eq!(find_in(dir.path()), Some(lib));
    }

    #[test]
    fn ignores_directories_with_the_library_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(LIBRARY_NAMES[0])).unwrap();
        assert!(find_in(dir.path()).is_none());
    }
}
