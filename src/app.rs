use crate::index::{IndexEvent, build_index, spawn_refresh};
use crate::parse::{LaunchResult, classify_launch, socket_unavailable};
use crate::run::run_capture;
use crate::notes::{self, EditError, Note};
use crate::store::{AppEntry, ButtonShape, ButtonStyle, Config, Paths, State, parse_palette, read_json, write_atomic};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Matcher, Utf32Str};
use ratatui::layout::Rect;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

pub const MAX_RECENT_SHOWN: usize = 8;
const MAX_RECENT_KEPT: usize = 20;
pub const MAX_HOME_ROWS: u16 = 8;
/// A swipe arrives as a burst of wheel events; after a page switch, ignore the rest of it.
const SWIPE_COOLDOWN: Duration = Duration::from_millis(500);
pub const OVERLAY_MSG: &str =
    "Grant Termux \"Display over other apps\": Settings > Apps > Termux > Display over other apps";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Apps,
    Notes,
    /// Opened from the Apps page; not a tab.
    Settings,
}

impl Page {
    /// Pages in the tab bar, in order.
    pub const ALL: [Page; 3] = [Page::Home, Page::Apps, Page::Notes];

    pub fn title(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Apps => "Apps",
            Page::Notes => "Notes",
            Page::Settings => "Settings",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Launch(String),
    TogglePin(String),
    Goto(Page),
    AdjustHomeRows(i16),
    SetPinnedStyle(ButtonStyle),
    SetListStyle(ButtonStyle),
    SetPinnedShape(ButtonShape),
    SetListShape(ButtonShape),
    SetIcons(bool),
    ToggleNote,
    /// Load the note at this file line into the prompt for editing.
    EditNote(usize),
    /// First tap arms, second tap on the same note deletes.
    DeleteNote(usize),
    /// Tap in the prompt text: the column where the visible text starts, and the char index shown
    /// there (the text scrolls when longer than the box).
    PromptAt(u16, usize),
}

pub struct App {
    pub entries: Vec<AppEntry>,
    pub state: State,
    pub paths: Paths,
    pub page: Page,
    /// Pinned apps the current page's grid can show (set by the renderer).
    pub grid_cap: usize,
    /// Static ASCII art for the Home page from `art.txt`; without it Home shows a spinning katana.
    pub art: Option<String>,
    /// RGB of the terminal's 16 colors, when known (Termux `colors.properties`).
    pub palette: [Option<(u8, u8, u8)>; 16],
    /// Drives the Home animation.
    pub started: Instant,
    pub config: Config,
    /// The prompt writes a note instead of searching.
    pub note_mode: bool,
    /// The note being edited in the prompt (note mode is on too).
    pub editing: Option<Note>,
    /// Notes page contents, newest first.
    pub notes: Vec<Note>,
    /// File line of the note whose delete button was tapped once.
    pub confirm_delete: Option<usize>,
    /// Cursor position in `query`, in chars.
    pub cursor: usize,
    last_switch: Option<Instant>,
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
        let mut config: Config = read_json(&paths.config).unwrap_or_default();
        config.home_rows = config.home_rows.clamp(1, MAX_HOME_ROWS);
        let palette = paths
            .termux_colors
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map_or([None; 16], |s| parse_palette(&s));
        let art = std::fs::read_to_string(&paths.art).ok().map(|s| s.trim_end().to_string());
        let mut app = Self {
            entries,
            state,
            paths,
            page: Page::Home,
            grid_cap: usize::MAX,
            art,
            started: Instant::now(),
            palette,
            config,
            note_mode: false,
            editing: None,
            notes: Vec::new(),
            confirm_delete: None,
            cursor: 0,
            last_switch: None,
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
        !self.note_mode && !self.query.is_empty()
    }

    /// Switch the prompt between search and note. Typed text carries over either way, so a search
    /// can be turned into a note; leaving an edit drops the note's text instead.
    pub fn toggle_note_mode(&mut self) {
        if self.editing.is_some() {
            self.cancel_note();
            return;
        }
        self.note_mode = !self.note_mode;
        self.reset_view();
    }

    /// Leave note/edit mode, dropping the prompt text.
    pub fn cancel_note(&mut self) {
        self.note_mode = false;
        self.editing = None;
        self.clear_query();
    }

    fn notes_path(&self) -> std::path::PathBuf {
        notes::expand(&self.config.notes_path)
    }

    pub fn reload_notes(&mut self) {
        match notes::load(&self.notes_path()) {
            Ok(mut n) => {
                n.reverse();
                self.notes = n;
            }
            Err(e) => self.status = format!("Cannot read notes: {e}"),
        }
        self.selected = self.selected.min(self.notes.len().saturating_sub(1));
    }

    pub fn edit_note(&mut self, line: usize) {
        let Some(note) = self.notes.iter().find(|n| n.line == line).cloned() else { return };
        self.confirm_delete = None;
        self.note_mode = true;
        self.query = note.text.clone();
        self.cursor = self.query.chars().count();
        self.editing = Some(note);
    }

    pub fn edit_selected_note(&mut self) {
        if let Some(line) = self.notes.get(self.selected).map(|n| n.line) {
            self.edit_note(line);
        }
    }

    pub fn delete_note(&mut self, line: usize) {
        if self.confirm_delete != Some(line) {
            self.confirm_delete = Some(line);
            self.status = "Tap again to delete".into();
            return;
        }
        self.confirm_delete = None;
        let Some(note) = self.notes.iter().find(|n| n.line == line).cloned() else { return };
        self.status = match notes::edit(&self.notes_path(), &note, None) {
            Ok(()) => "Note deleted".into(),
            Err(e) => edit_error(e),
        };
        if self.editing.as_ref().is_some_and(|n| n.line == line) {
            self.cancel_note();
        }
        self.reload_notes();
    }

    pub fn save_note(&mut self) {
        let text = self.query.trim().to_string();
        if text.is_empty() {
            return;
        }
        let path = self.notes_path();
        if let Some(note) = self.editing.clone() {
            match notes::edit(&path, &note, Some(&text)) {
                Ok(()) => {
                    self.status = "Note saved".into();
                    self.cancel_note();
                }
                // Keep the edited text in the prompt; the list shows the file as it is now.
                Err(e) => {
                    self.status = edit_error(e);
                    self.editing = None;
                }
            }
            self.reload_notes();
            return;
        }
        match notes::append(&path, &text) {
            Ok(()) => {
                let name = path.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                self.status = format!("Noted to {name}");
                self.cancel_note();
                if self.page == Page::Notes {
                    self.reload_notes();
                }
            }
            // The text stays in the prompt so it isn't lost.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                self.status = "Notes folder missing: run termux-setup-storage".into()
            }
            Err(e) => self.status = format!("Cannot save note: {e}"),
        }
    }

    pub fn find(&self, pkg: &str) -> Option<&AppEntry> {
        self.entries.iter().find(|e| e.pkg == pkg)
    }

    /// The current page's pin list.
    fn pins(&self) -> &[String] {
        match self.page {
            Page::Home => &self.state.pinned,
            Page::Apps => &self.state.pinned_apps,
            Page::Notes | Page::Settings => &[],
        }
    }

    fn pins_mut(&mut self) -> Option<&mut Vec<String>> {
        match self.page {
            Page::Home => Some(&mut self.state.pinned),
            Page::Apps => Some(&mut self.state.pinned_apps),
            Page::Notes | Page::Settings => None,
        }
    }

    /// Current page's pinned apps that are installed but don't fit the grid.
    pub fn pinned_hidden(&self) -> usize {
        self.pins().iter().filter(|p| self.find(p).is_some()).count().saturating_sub(self.grid_cap)
    }

    /// Pinned on the current page.
    pub fn is_pinned(&self, pkg: &str) -> bool {
        self.pins().iter().any(|p| p == pkg)
    }

    /// Current page's pinned apps that are still installed and fit the grid, in pin order.
    pub fn pinned(&self) -> Vec<&AppEntry> {
        self.pins().iter().filter_map(|p| self.find(p)).take(self.grid_cap).collect()
    }

    /// Recent apps (Apps page only): installed, not pinned on any page, at most `MAX_RECENT_SHOWN`.
    pub fn recent(&self) -> Vec<&AppEntry> {
        if self.page != Page::Apps {
            return Vec::new();
        }
        let pinned_anywhere = |p: &String| self.state.pinned.contains(p) || self.state.pinned_apps.contains(p);
        self.state
            .recent
            .iter()
            .filter(|p| !pinned_anywhere(p))
            .filter_map(|p| self.find(p))
            .take(MAX_RECENT_SHOWN)
            .collect()
    }

    /// Items the selection moves over: search results, or the page's pinned then recent.
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

    /// Byte offset of char position `pos` in `query`.
    fn byte_at(&self, pos: usize) -> usize {
        self.query.char_indices().nth(pos).map_or(self.query.len(), |(i, _)| i)
    }

    pub fn type_char(&mut self, c: char) {
        let at = self.byte_at(self.cursor);
        self.query.insert(at, c);
        self.cursor += 1;
        self.reset_view();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        self.cursor -= 1;
        let at = self.byte_at(self.cursor);
        self.query.remove(at);
        self.reset_view();
    }

    pub fn move_cursor(&mut self, delta: isize) {
        let n = self.query.chars().count() as isize;
        self.cursor = (self.cursor as isize + delta).clamp(0, n) as usize;
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.cursor = 0;
        self.reset_view();
    }

    fn reset_view(&mut self) {
        self.selected = 0;
        self.scroll = 0;
        self.follow_selection = true;
    }

    pub fn goto(&mut self, page: Page) {
        if self.page != page {
            self.page = page;
            self.grid_cap = usize::MAX;
            self.confirm_delete = None;
            self.reset_view();
            if page == Page::Notes {
                self.reload_notes();
            }
        }
    }

    /// Switch to the next (`delta` > 0) or previous page, wrapping around.
    pub fn cycle_page(&mut self, delta: isize) {
        let n = Page::ALL.len() as isize;
        // Settings sits under Apps.
        let i = Page::ALL.iter().position(|p| *p == self.page).unwrap_or(1) as isize;
        self.goto(Page::ALL[(i + delta).rem_euclid(n) as usize]);
    }

    /// Mouse wheel (Termux sends vertical swipes as wheel events). Outside search, swiping up on
    /// Home opens Apps and swiping down at the top of Apps returns Home; otherwise it scrolls.
    pub fn wheel(&mut self, delta: isize) {
        if self.last_switch.is_some_and(|t| t.elapsed() < SWIPE_COOLDOWN) {
            return;
        }
        if !self.searching() {
            let target = match self.page {
                Page::Home if delta > 0 => Some(Page::Apps),
                Page::Apps if delta < 0 && self.scroll == 0 => Some(Page::Home),
                _ => None,
            };
            if let Some(page) = target {
                self.goto(page);
                self.last_switch = Some(Instant::now());
                return;
            }
        }
        self.scroll_by(delta);
    }

    /// Rows the selection and scrolling move over on the current view.
    fn selectable(&self) -> usize {
        if self.page == Page::Notes && !self.searching() { self.notes.len() } else { self.items().len() }
    }

    pub fn move_selection(&mut self, delta: isize) {
        let n = self.selectable();
        if n == 0 {
            return;
        }
        self.selected = (self.selected as isize + delta).clamp(0, n as isize - 1) as usize;
        self.follow_selection = true;
    }

    pub fn scroll_by(&mut self, delta: isize) {
        let n = self.selectable();
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
        if !matches!(hit, Some(Action::DeleteNote(_))) {
            self.confirm_delete = None;
        }
        match hit {
            Some(Action::EditNote(line)) => self.edit_note(line),
            Some(Action::DeleteNote(line)) => self.delete_note(line),
            Some(Action::PromptAt(x0, start)) => {
                let n = self.query.chars().count();
                self.cursor = (start + col.saturating_sub(x0) as usize).min(n);
            }
            Some(Action::Launch(pkg)) => self.launch(&pkg),
            Some(Action::TogglePin(pkg)) => self.toggle_pin(&pkg),
            Some(Action::Goto(page)) => self.goto(page),
            Some(Action::AdjustHomeRows(d)) => self.adjust_home_rows(d),
            Some(Action::SetPinnedStyle(s)) => {
                self.config.pinned_style = s;
                self.save_config();
            }
            Some(Action::SetListStyle(s)) => {
                self.config.list_style = s;
                self.save_config();
            }
            Some(Action::SetPinnedShape(s)) => {
                self.config.pinned_shape = s;
                self.save_config();
            }
            Some(Action::SetListShape(s)) => {
                self.config.list_shape = s;
                self.save_config();
            }
            Some(Action::ToggleNote) => self.toggle_note_mode(),
            Some(Action::SetIcons(on)) => {
                self.config.icons = on;
                self.save_config();
            }
            None => {}
        }
    }

    // ---- actions ----

    pub fn toggle_pin(&mut self, pkg: &str) {
        let Some(pins) = self.pins_mut() else { return };
        if let Some(i) = pins.iter().position(|p| p == pkg) {
            pins.remove(i);
        } else {
            pins.push(pkg.to_string());
        }
        let label = self.find(pkg).map_or(pkg, |e| &e.label);
        self.status = if !self.is_pinned(pkg) {
            format!("Unpinned {label} from {}", self.page.title())
        } else if self.pinned_hidden() > 0 {
            format!("Pinned {label}, but {} is full: it won't be shown", self.page.title())
        } else {
            format!("Pinned {label} to {}", self.page.title())
        };
        self.save_state();
        let n = self.items().len();
        self.selected = self.selected.min(n.saturating_sub(1));
    }

    pub fn adjust_home_rows(&mut self, delta: i16) {
        let rows = (self.config.home_rows as i16 + delta).clamp(1, MAX_HOME_ROWS as i16) as u16;
        if rows == self.config.home_rows {
            return;
        }
        self.config.home_rows = rows;
        self.save_config();
    }

    fn save_config(&mut self) {
        if let Err(e) = write_atomic(&self.paths.config, &self.config) {
            self.status = format!("Cannot save config: {e}");
        }
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
        match result {
            LaunchResult::Success | LaunchResult::BroughtToFront => {
                self.state.recent.retain(|p| p != pkg);
                self.state.recent.insert(0, pkg.to_string());
                self.state.recent.truncate(MAX_RECENT_KEPT);
                self.save_state();
                self.clear_query();
                self.status = format!("Launched {}", entry.label);
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
                IndexEvent::Updated(entries) => self.entries = entries,
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

fn edit_error(e: EditError) -> String {
    match e {
        EditError::Changed => "Note changed elsewhere; reloaded".into(),
        EditError::Io(e) => format!("Cannot save note: {e}"),
    }
}

/// Fuzzy match `query` against label and package; best score wins, ties by label.
pub fn search<'a>(entries: &'a [AppEntry], query: &str) -> Vec<&'a AppEntry> {
    let mut matcher = Matcher::new(nucleo_matcher::Config::DEFAULT);
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
    fn swipe_switches_pages_once_per_burst() {
        let dir = std::env::temp_dir().join(format!("termdeck-swipe-{}", std::process::id()));
        let mut app = App::new(Paths::in_dirs(&dir.join("c"), &dir.join("d"), &dir.join("e")));
        app.wheel(1);
        assert_eq!(app.page, Page::Apps);
        // The rest of the same swipe must not bounce back.
        app.wheel(-1);
        assert_eq!(app.page, Page::Apps);
        app.last_switch = None;
        app.wheel(-1);
        assert_eq!(app.page, Page::Home);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn notes_edit_and_double_tap_delete() {
        let dir = std::env::temp_dir().join(format!("termdeck-app-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("n.md");
        std::fs::write(&file, "- 2026-10-11 10:00 first\n- 2026-10-11 11:00 second\n").unwrap();
        let mut app = App::new(Paths::in_dirs(&dir.join("c"), &dir.join("d"), &dir.join("e")));
        app.config.notes_path = file.to_string_lossy().into_owned();
        app.goto(Page::Notes);
        assert_eq!(app.notes[0].text, "second"); // newest first

        app.edit_note(0);
        assert_eq!((app.query.as_str(), app.cursor), ("first", 5));
        app.move_cursor(-5);
        for c in "the ".chars() {
            app.type_char(c);
        }
        app.save_note();
        assert!(app.editing.is_none() && !app.note_mode);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "- 2026-10-11 10:00 the first\n- 2026-10-11 11:00 second\n");

        app.delete_note(1);
        assert_eq!(app.notes.len(), 2, "first tap only arms");
        app.delete_note(1);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "- 2026-10-11 10:00 the first\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn note_toggle_keeps_typed_text() {
        let dir = std::env::temp_dir().join(format!("termdeck-app-toggle-{}", std::process::id()));
        let mut app = App::new(Paths::in_dirs(&dir.join("c"), &dir.join("d"), &dir.join("e")));
        for c in "buy milk".chars() {
            app.type_char(c);
        }
        app.toggle_note_mode();
        assert!(app.note_mode && !app.searching());
        assert_eq!((app.query.as_str(), app.cursor), ("buy milk", 8));
        app.toggle_note_mode();
        assert!(app.searching());
        let _ = std::fs::remove_dir_all(&dir);
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
