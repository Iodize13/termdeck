use crate::index::{BuildStats, IndexEvent, build_index, spawn_refresh};
use crate::parse::{LaunchResult, classify_launch, socket_unavailable};
use crate::run::run_capture;
use crate::store::{AppEntry, Paths, State, read_json, write_atomic};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::layout::Rect;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

pub const MAX_RECENT_SHOWN: usize = 8;
const MAX_RECENT_KEPT: usize = 20;
pub const OVERLAY_MSG: &str =
    "Grant Termux \"Display over other apps\": Settings > Apps > Termux > Display over other apps";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Launch(String),
    TogglePin(String),
}

pub struct App {
    pub entries: Vec<AppEntry>,
    pub state: State,
    pub paths: Paths,
    pub query: String,
    pub selected: usize,
    pub scroll: usize,
    /// Keep the selection in view on the next render (set by keyboard navigation).
    pub follow_selection: bool,
    pub status: String,
    pub refreshing: bool,
    pub should_quit: bool,
    /// Hit targets recorded during the last render.
    pub hits: Vec<(Rect, Action)>,
    /// List rows visible in the last render (for scrolling limits).
    pub visible_rows: usize,
    tx: Sender<IndexEvent>,
    rx: Receiver<IndexEvent>,
}

impl App {
    pub fn new(paths: Paths) -> Self {
        let (tx, rx) = channel();
        let entries: Vec<AppEntry> = read_json(&paths.index).unwrap_or_default();
        let state: State = read_json(&paths.state).unwrap_or_default();
        let mut app = Self {
            entries,
            state,
            paths,
            query: String::new(),
            selected: 0,
            scroll: 0,
            follow_selection: true,
            status: String::new(),
            refreshing: false,
            should_quit: false,
            hits: Vec::new(),
            visible_rows: 0,
            tx,
            rx,
        };
        app.refresh();
        app
    }

    pub fn searching(&self) -> bool {
        !self.query.is_empty()
    }

    pub fn find(&self, pkg: &str) -> Option<&AppEntry> {
        self.entries.iter().find(|e| e.pkg == pkg)
    }

    pub fn is_pinned(&self, pkg: &str) -> bool {
        self.state.pinned.iter().any(|p| p == pkg)
    }

    /// Pinned apps that are still installed, in pin order.
    pub fn pinned(&self) -> Vec<&AppEntry> {
        self.state.pinned.iter().filter_map(|p| self.find(p)).collect()
    }

    /// Recent apps: installed, not pinned, at most `MAX_RECENT_SHOWN`.
    pub fn recent(&self) -> Vec<&AppEntry> {
        self.state
            .recent
            .iter()
            .filter(|p| !self.is_pinned(p))
            .filter_map(|p| self.find(p))
            .take(MAX_RECENT_SHOWN)
            .collect()
    }

    /// Items the selection moves over: search results, or pinned then recent.
    pub fn items(&self) -> Vec<&AppEntry> {
        if self.searching() {
            search(&self.entries, &self.query)
        } else {
            let mut v = self.pinned();
            v.extend(self.recent());
            v
        }
    }

    pub fn selected_entry(&self) -> Option<AppEntry> {
        self.items().get(self.selected).map(|e| (*e).clone())
    }

    // ---- input ----

    pub fn type_char(&mut self, c: char) {
        self.query.push(c);
        self.reset_view();
    }

    pub fn backspace(&mut self) {
        self.query.pop();
        self.reset_view();
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.reset_view();
    }

    fn reset_view(&mut self) {
        self.selected = 0;
        self.scroll = 0;
        self.follow_selection = true;
    }

    pub fn move_selection(&mut self, delta: isize) {
        let n = self.items().len();
        if n == 0 {
            return;
        }
        self.selected = (self.selected as isize + delta).clamp(0, n as isize - 1) as usize;
        self.follow_selection = true;
    }

    pub fn scroll_by(&mut self, delta: isize) {
        let n = self.items().len();
        let max = n.saturating_sub(self.visible_rows.max(1));
        self.scroll = (self.scroll as isize + delta).clamp(0, max as isize) as usize;
        self.follow_selection = false;
    }

    pub fn click(&mut self, col: u16, row: u16) {
        let hit = self
            .hits
            .iter()
            .rev()
            .find(|(r, _)| col >= r.x && col < r.x + r.width && row >= r.y && row < r.y + r.height)
            .map(|(_, a)| a.clone());
        match hit {
            Some(Action::Launch(pkg)) => self.launch(&pkg),
            Some(Action::TogglePin(pkg)) => self.toggle_pin(&pkg),
            None => {}
        }
    }

    // ---- actions ----

    pub fn toggle_pin(&mut self, pkg: &str) {
        if let Some(i) = self.state.pinned.iter().position(|p| p == pkg) {
            self.state.pinned.remove(i);
        } else {
            self.state.pinned.push(pkg.to_string());
        }
        self.save_state();
        let n = self.items().len();
        self.selected = self.selected.min(n.saturating_sub(1));
    }

    pub fn toggle_pin_selected(&mut self) {
        if let Some(e) = self.selected_entry() {
            self.toggle_pin(&e.pkg);
        }
    }

    pub fn launch_selected(&mut self) {
        if let Some(e) = self.selected_entry() {
            self.launch(&e.pkg);
        }
    }

    pub fn launch(&mut self, pkg: &str) {
        let Some(entry) = self.find(pkg).cloned() else { return };
        let t = Instant::now();
        let start = |program: &str| {
            run_capture(program, &["start", "-n", &entry.activity])
                .map(|out| {
                    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
                    text.push_str(&String::from_utf8_lossy(&out.stderr));
                    text
                })
                .map_err(|e| format!("{program}: {e}"))
        };
        // termux-am needs termux-app's socket server; fall back to the slower
        // `am` (dalvik) when the server isn't running or termux-am is missing.
        let text = match start("termux-am") {
            Ok(t) if !socket_unavailable(&t) => Ok(t),
            _ => start("am"),
        };
        let result = match text {
            Ok(t) => classify_launch(&t),
            Err(e) => LaunchResult::Failed(e),
        };
        let ms = t.elapsed().as_millis();
        match result {
            LaunchResult::Success | LaunchResult::BroughtToFront => {
                self.state.recent.retain(|p| p != pkg);
                self.state.recent.insert(0, pkg.to_string());
                self.state.recent.truncate(MAX_RECENT_KEPT);
                self.save_state();
                self.clear_query();
                self.status = format!("Launched {} ({ms} ms)", entry.label);
            }
            LaunchResult::NeedsOverlayPermission => self.status = OVERLAY_MSG.to_string(),
            LaunchResult::Failed(msg) => {
                self.status = format!("Launch failed: {msg}");
                if msg.contains("does not exist") || msg.contains("not found") {
                    self.refresh();
                }
            }
        }
    }

    pub fn refresh(&mut self) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        let old = self.entries.clone();
        spawn_refresh(self.tx.clone(), self.paths.clone(), move || build_index(&old));
    }

    pub fn poll_index(&mut self) {
        while let Ok(ev) = self.rx.try_recv() {
            self.refreshing = false;
            match ev {
                IndexEvent::Updated(entries, BuildStats { aapt_reads, fallbacks }) => {
                    self.entries = entries;
                    self.status = format!(
                        "Index: {} apps ({aapt_reads} labels read, {fallbacks} fallback)",
                        self.entries.len()
                    );
                }
                IndexEvent::Failed(msg) => self.status = format!("Index refresh failed: {msg}"),
            }
        }
    }

    fn save_state(&mut self) {
        if let Err(e) = write_atomic(&self.paths.state, &self.state) {
            self.status = format!("Cannot save state: {e}");
        }
    }
}

/// Fuzzy match `query` against label and package; best score wins, ties by label.
pub fn search<'a>(entries: &'a [AppEntry], query: &str) -> Vec<&'a AppEntry> {
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);
    let mut buf = Vec::new();
    let mut scored: Vec<(u32, &AppEntry)> = entries
        .iter()
        .filter_map(|e| {
            let a = pattern.score(Utf32Str::new(&e.label, &mut buf), &mut matcher);
            let b = pattern.score(Utf32Str::new(&e.pkg, &mut buf), &mut matcher);
            a.max(b).map(|s| (s, e))
        })
        .collect();
    scored.sort_by(|(sa, a), (sb, b)| sb.cmp(sa).then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase())));
    scored.into_iter().map(|(_, e)| e).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(pkg: &str, label: &str) -> AppEntry {
        AppEntry { pkg: pkg.into(), activity: format!("{pkg}/.Main"), label: label.into(), apk_mtime: 1 }
    }

    #[test]
    fn fuzzy_ranks_best_first() {
        let apps = vec![
            e("com.android.settings", "Settings"),
            e("app.revanced.youtube", "YouTube Morphe"),
            e("com.google.music", "YT Music Morphe"),
            e("com.example.calc", "Calculator"),
        ];
        let r = search(&apps, "yt");
        assert!(r.len() >= 2);
        let top: Vec<&str> = r.iter().take(2).map(|a| a.label.as_str()).collect();
        assert!(top.contains(&"YT Music Morphe") && top.contains(&"YouTube Morphe"), "{top:?}");
        assert!(search(&apps, "zzzq").is_empty());
    }
}
