use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppEntry {
    pub pkg: String,
    /// Full component, `pkg/activity`.
    pub activity: String,
    pub label: String,
    pub apk_mtime: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub pinned: Vec<String>,
    #[serde(default)]
    pub recent: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub index: PathBuf,
    pub state: PathBuf,
}

impl Paths {
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
        let dir = |var: &str, fallback: &[&str]| {
            match std::env::var_os(var).filter(|v| !v.is_empty()) {
                Some(v) => PathBuf::from(v),
                None => fallback.iter().fold(home.clone(), |p, s| p.join(s)),
            }
        };
        Self::in_dirs(&dir("XDG_CACHE_HOME", &[".cache"]), &dir("XDG_DATA_HOME", &[".local", "share"]))
    }

    pub fn in_dirs(cache: &Path, data: &Path) -> Self {
        Self {
            index: cache.join("termdeck").join("index.json"),
            state: data.join("termdeck").join("state.json"),
        }
    }
}

/// Write via temp file + rename so an interrupted write cannot corrupt the file.
pub fn write_atomic<T: Serialize>(path: &Path, value: &T) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec(value)?)?;
    std::fs::rename(&tmp, path)
}

/// Missing or corrupt file → `None`.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("termdeck-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = Paths::in_dirs(&dir.join("cache"), &dir.join("data"));

        let idx = vec![AppEntry {
            pkg: "a.b".into(),
            activity: "a.b/a.b.Main".into(),
            label: "B".into(),
            apk_mtime: 42,
        }];
        let st = State { pinned: vec!["a.b".into()], recent: vec!["c.d".into(), "a.b".into()] };

        write_atomic(&paths.index, &idx).unwrap();
        write_atomic(&paths.state, &st).unwrap();
        assert_eq!(read_json::<Vec<AppEntry>>(&paths.index), Some(idx));
        assert_eq!(read_json::<State>(&paths.state), Some(st));
        assert!(!paths.state.with_extension("json.tmp").exists());
        assert_eq!(read_json::<State>(&dir.join("missing.json")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
