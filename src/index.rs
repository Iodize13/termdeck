use crate::parse::{label_fallback, parse_badging_label, parse_pm_path, parse_query_activities};
use crate::run::{run_capture, run_stdout};
use crate::store::{AppEntry, Paths, write_atomic};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

const WORKERS: usize = 4;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BuildStats {
    /// Labels read through `aapt` (new or changed APKs).
    pub aapt_reads: usize,
    /// Of those, how many fell back to the package-name label.
    pub fallbacks: usize,
}

#[derive(Debug)]
pub enum IndexEvent {
    Updated(Vec<AppEntry>, BuildStats),
    Failed(String),
}

type BuildResult = Result<(Vec<AppEntry>, BuildStats), String>;

/// Discover launchable apps, reusing labels of `old` entries whose APK mtime is unchanged.
pub fn build_index(old: &[AppEntry]) -> BuildResult {
    let out = run_stdout(
        "pm",
        &[
            "query-activities", "--user", "0", "--brief",
            "-a", "android.intent.action.MAIN", "-c", "android.intent.category.LAUNCHER",
        ],
    )?;
    let found = parse_query_activities(&out);
    if found.is_empty() {
        return Err("no launcher activities found".into());
    }
    let old: HashMap<&str, &AppEntry> = old.iter().map(|e| (e.pkg.as_str(), e)).collect();

    let next = AtomicUsize::new(0);
    let aapt_reads = AtomicUsize::new(0);
    let fallbacks = AtomicUsize::new(0);
    let results: Mutex<Vec<AppEntry>> = Mutex::new(Vec::with_capacity(found.len()));

    std::thread::scope(|s| {
        for _ in 0..WORKERS.min(found.len()) {
            s.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((pkg, activity)) = found.get(i) else { break };
                let apk = run_stdout("pm", &["path", pkg]).ok().and_then(|o| parse_pm_path(&o));
                let mtime = apk.as_deref().map(apk_mtime).unwrap_or(0);
                let label = match old.get(pkg.as_str()) {
                    Some(o) if mtime != 0 && o.apk_mtime == mtime => o.label.clone(),
                    _ => {
                        aapt_reads.fetch_add(1, Ordering::Relaxed);
                        let l = apk.as_deref().and_then(read_label);
                        l.unwrap_or_else(|| {
                            fallbacks.fetch_add(1, Ordering::Relaxed);
                            label_fallback(pkg)
                        })
                    }
                };
                results.lock().unwrap().push(AppEntry {
                    pkg: pkg.clone(),
                    activity: activity.clone(),
                    label,
                    apk_mtime: mtime,
                });
            });
        }
    });

    let mut entries = results.into_inner().unwrap();
    entries.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()).then(a.pkg.cmp(&b.pkg)));
    let stats = BuildStats {
        aapt_reads: aapt_reads.into_inner(),
        fallbacks: fallbacks.into_inner(),
    };
    Ok((entries, stats))
}

fn apk_mtime(path: &str) -> u64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn read_label(apk: &str) -> Option<String> {
    // aapt exits 1 on non-fatal manifest errors yet still prints the label.
    let out = run_capture("aapt", &["dump", "badging", apk]).ok()?;
    parse_badging_label(&String::from_utf8_lossy(&out.stdout))
}

/// Run `discover` on a worker thread. On success save the index and send `Updated`;
/// on failure send `Failed` and leave the on-disk index untouched.
pub fn spawn_refresh<F>(tx: Sender<IndexEvent>, paths: Paths, discover: F)
where
    F: FnOnce() -> BuildResult + Send + 'static,
{
    std::thread::spawn(move || {
        let ev = match discover() {
            Ok((entries, stats)) => {
                if let Err(e) = write_atomic(&paths.index, &entries) {
                    IndexEvent::Failed(format!("cannot save index: {e}"))
                } else {
                    IndexEvent::Updated(entries, stats)
                }
            }
            Err(e) => IndexEvent::Failed(e),
        };
        let _ = tx.send(ev);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::read_json;

    #[test]
    fn failure_keeps_old_index() {
        let dir = std::env::temp_dir().join(format!("termdeck-idx-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = Paths::in_dirs(&dir.join("c"), &dir.join("d"));
        let old = vec![AppEntry { pkg: "a".into(), activity: "a/A".into(), label: "A".into(), apk_mtime: 1 }];
        write_atomic(&paths.index, &old).unwrap();

        let (tx, rx) = std::sync::mpsc::channel();
        spawn_refresh(tx, paths.clone(), || Err("boom".to_string()));
        match rx.recv().unwrap() {
            IndexEvent::Failed(m) => assert_eq!(m, "boom"),
            other => panic!("unexpected {other:?}"),
        }
        assert_eq!(read_json::<Vec<AppEntry>>(&paths.index), Some(old));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn success_saves_and_sends() {
        let dir = std::env::temp_dir().join(format!("termdeck-idx2-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let paths = Paths::in_dirs(&dir.join("c"), &dir.join("d"));
        let new = vec![AppEntry { pkg: "b".into(), activity: "b/B".into(), label: "B".into(), apk_mtime: 2 }];
        let n2 = new.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        spawn_refresh(tx, paths.clone(), move || Ok((n2, BuildStats::default())));
        assert!(matches!(rx.recv().unwrap(), IndexEvent::Updated(e, _) if e == new));
        assert_eq!(read_json::<Vec<AppEntry>>(&paths.index), Some(new));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
