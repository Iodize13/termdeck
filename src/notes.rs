//! Quick notes: one line per note appended to a markdown file. By default it lives in termdeck's
//! private data dir; point `notes_path` at shared storage to sync it (e.g. with Syncthing).

use crate::run::run_stdout;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

/// `~/x` → `$HOME/x`.
pub fn expand(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest),
        _ => PathBuf::from(path),
    }
}

/// Local "YYYY-MM-DD HH:MM" from `date`, which on Termux follows the Android time zone.
fn now() -> String {
    run_stdout("date", &["+%Y-%m-%d %H:%M"]).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// Append `- <time> <text>` to `path`, creating its directories. Newlines in `text` become spaces so
/// each note stays one line.
pub fn append(path: &Path, text: &str) -> std::io::Result<()> {
    append_at(path, text, &now())
}

fn append_at(path: &Path, text: &str, time: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty() && !d.exists()) {
        // Creating ~/storage here would make it a plain directory that termux-setup-storage
        // then can't link, so require that step instead.
        if let Some(home) = std::env::var_os("HOME") {
            let storage = Path::new(&home).join("storage");
            if dir.starts_with(&storage) && !storage.exists() {
                return Err(std::io::Error::new(ErrorKind::NotFound, "~/storage missing: run termux-setup-storage"));
            }
        }
        std::fs::create_dir_all(dir)?;
    }
    let line = format_line(time, text) + "\n";
    std::fs::OpenOptions::new().create(true).append(true).open(path)?.write_all(line.as_bytes())
}

fn format_line(time: &str, text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if time.is_empty() { format!("- {text}") } else { format!("- {time} {text}") }
}

/// One `- ` line of the notes file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// 0-based line number in the file.
    pub line: usize,
    /// The line as read, to detect edits made elsewhere before writing.
    pub raw: String,
    /// "YYYY-MM-DD HH:MM", or empty when the line has none.
    pub time: String,
    pub text: String,
}

/// Notes in file order. Lines that aren't `- ` items (headings, blank lines…) are skipped here and
/// left untouched by edits. A missing file has no notes.
pub fn load(path: &Path) -> std::io::Result<Vec<Note>> {
    let content = match std::fs::read_to_string(path) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        r => r?,
    };
    Ok(content.lines().enumerate().filter_map(|(line, raw)| parse(line, raw)).collect())
}

fn parse(line: usize, raw: &str) -> Option<Note> {
    let rest = raw.strip_prefix("- ")?;
    // "YYYY-MM-DD HH:MM " is 17 bytes; check the shape so a note starting with a number isn't cut.
    let has_time = rest.len() > 17
        && rest.as_bytes()[..16].iter().enumerate().all(|(i, b)| match i {
            4 | 7 => *b == b'-',
            10 => *b == b' ',
            13 => *b == b':',
            _ => b.is_ascii_digit(),
        })
        && rest.as_bytes()[16] == b' ';
    let (time, text) = if has_time { (&rest[..16], &rest[17..]) } else { ("", rest) };
    Some(Note { line, raw: raw.to_string(), time: time.to_string(), text: text.to_string() })
}

#[derive(Debug)]
pub enum EditError {
    /// The line no longer matches what was loaded (changed by sync or another editor).
    Changed,
    Io(std::io::Error),
}

/// Replace `note`'s line with `text` (keeping its time), or delete it when `text` is `None`. The
/// file is rewritten through a temp file + rename, so a crash can't leave it half-written.
pub fn edit(path: &Path, note: &Note, text: Option<&str>) -> Result<(), EditError> {
    let content = std::fs::read_to_string(path).map_err(EditError::Io)?;
    let mut lines: Vec<&str> = content.lines().collect();
    if lines.get(note.line) != Some(&note.raw.as_str()) {
        return Err(EditError::Changed);
    }
    let new_line;
    match text {
        Some(t) => {
            new_line = format_line(&note.time, t);
            lines[note.line] = &new_line;
        }
        None => {
            lines.remove(note.line);
        }
    }
    let mut out = lines.join("\n");
    if !out.is_empty() {
        out.push('\n');
    }
    let tmp = path.with_extension("md.tmp");
    std::fs::write(&tmp, out).and_then(|()| std::fs::rename(&tmp, path)).map_err(EditError::Io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_lines() {
        let dir = std::env::temp_dir().join(format!("termdeck-notes-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a").join("b").join("mobile.md");
        append_at(&path, "buy  milk\nand eggs", "2026-10-11 17:30").unwrap();
        append_at(&path, "call mom", "").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "- 2026-10-11 17:30 buy milk and eggs\n- call mom\n");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn loads_edits_and_deletes() {
        let dir = std::env::temp_dir().join(format!("termdeck-notes-edit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("n.md");
        std::fs::write(&path, "# Inbox\n- 2026-10-11 17:30 buy milk\n\n- 2026 plans\n- 2026-10-12 08:00 call\n").unwrap();

        let notes = load(&path).unwrap();
        let summary: Vec<_> = notes.iter().map(|n| (n.line, n.time.as_str(), n.text.as_str())).collect();
        assert_eq!(summary, [(1, "2026-10-11 17:30", "buy milk"), (3, "", "2026 plans"), (4, "2026-10-12 08:00", "call")]);

        edit(&path, &notes[0], Some("buy oat milk")).unwrap();
        edit(&path, &notes[2], None).unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "# Inbox\n- 2026-10-11 17:30 buy oat milk\n\n- 2026 plans\n");

        // notes[0] is stale now: its line changed.
        assert!(matches!(edit(&path, &notes[0], Some("x")), Err(EditError::Changed)));
        assert_eq!(load(&dir.join("missing.md")).unwrap(), Vec::new());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn expands_tilde() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand("~/a/b.md"), Path::new(&home).join("a/b.md"));
        assert_eq!(expand("/x/y.md"), PathBuf::from("/x/y.md"));
    }
}
