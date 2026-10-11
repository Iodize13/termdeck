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
    /// Pins on the Home page.
    #[serde(default)]
    pub pinned: Vec<String>,
    /// Pins on the Apps page.
    #[serde(default)]
    pub pinned_apps: Vec<String>,
    #[serde(default)]
    pub recent: Vec<String>,
}

/// How app buttons are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ButtonStyle {
    /// Slanted outline only; the background shows through.
    Outline,
    /// Slanted solid fill.
    Filled,
}

/// User settings, `config.json` in the config dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Grid rows of pinned apps on the Home page.
    pub home_rows: u16,
    /// Pinned-app grid buttons.
    pub pinned_style: ButtonStyle,
    /// Recent and search-result rows.
    pub list_style: ButtonStyle,
}

impl Default for Config {
    fn default() -> Self {
        Self { home_rows: 3, pinned_style: ButtonStyle::Outline, list_style: ButtonStyle::Filled }
    }
}

#[derive(Debug, Clone)]
pub struct Paths {
    pub index: PathBuf,
    pub state: PathBuf,
    /// Optional ASCII art shown on the Home page.
    pub art: PathBuf,
    pub config: PathBuf,
    /// Termux's color scheme, read to pick readable text on filled rows.
    pub termux_colors: Option<PathBuf>,
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
        let mut paths = Self::in_dirs(
            &dir("XDG_CACHE_HOME", &[".cache"]),
            &dir("XDG_DATA_HOME", &[".local", "share"]),
            &dir("XDG_CONFIG_HOME", &[".config"]),
        );
        paths.termux_colors = Some(home.join(".termux").join("colors.properties"));
        paths
    }

    pub fn in_dirs(cache: &Path, data: &Path, config: &Path) -> Self {
        Self {
            index: cache.join("termdeck").join("index.json"),
            state: data.join("termdeck").join("state.json"),
            art: config.join("termdeck").join("art.txt"),
            config: config.join("termdeck").join("config.json"),
            termux_colors: None,
        }
    }
}

/// `colorN=#rrggbb` entries of a Termux `colors.properties`, indexed by N (0–15).
pub fn parse_palette(text: &str) -> [Option<(u8, u8, u8)>; 16] {
    let mut out = [None; 16];
    for line in text.lines() {
        let Some((key, val)) = line.split_once('=') else { continue };
        let Some(n) = key.trim().strip_prefix("color").and_then(|n| n.parse::<usize>().ok()) else { continue };
        let hex = val.trim().trim_start_matches('#');
        if n < 16 && hex.len() == 6 {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                out[n] = Some(((v >> 16) as u8, (v >> 8) as u8, v as u8));
            }
        }
    }
    out
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
    fn parses_palette() {
        let p = parse_palette("background=#000000\ncolor1=#957A71\n color12 = #8875a8\ncolor99=#ffffff\ncolor3=bad");
        assert_eq!(p[1], Some((0x95, 0x7a, 0x71)));
        assert_eq!(p[12], Some((0x88, 0x75, 0xa8)));
        assert_eq!(p[3], None);
        assert_eq!(p[0], None);
    }

    #[test]
    fn roundtrip() {
        let dir = std::env::temp_dir().join(format!("termdeck-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = Paths::in_dirs(&dir.join("cache"), &dir.join("data"), &dir.join("config"));

        let idx = vec![AppEntry {
            pkg: "a.b".into(),
            activity: "a.b/a.b.Main".into(),
            label: "B".into(),
            apk_mtime: 42,
        }];
        let st = State {
            pinned: vec!["a.b".into()],
            pinned_apps: vec!["e.f".into()],
            recent: vec!["c.d".into(), "a.b".into()],
        };

        write_atomic(&paths.index, &idx).unwrap();
        write_atomic(&paths.state, &st).unwrap();
        assert_eq!(read_json::<Vec<AppEntry>>(&paths.index), Some(idx));
        assert_eq!(read_json::<State>(&paths.state), Some(st));
        assert!(!paths.state.with_extension("json.tmp").exists());
        assert_eq!(read_json::<State>(&dir.join("missing.json")), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
