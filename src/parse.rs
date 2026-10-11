//! Pure parsers for `pm` / `aapt` / `termux-am` output.

/// `pm query-activities --brief` → `(pkg, activity_component)`, first per package.
/// `activity_component` is the full `pkg/activity` string.
pub fn parse_query_activities(out: &str) -> Vec<(String, String)> {
    let mut seen = std::collections::HashSet::new();
    let mut res = Vec::new();
    for line in out.lines() {
        let line = line.trim();
        let Some((pkg, act)) = line.split_once('/') else { continue };
        if pkg.is_empty()
            || act.is_empty()
            || line.contains(char::is_whitespace)
            || line.contains('=')
        {
            continue;
        }
        if seen.insert(pkg.to_string()) {
            res.push((pkg.to_string(), line.to_string()));
        }
    }
    res
}

/// `pm path <pkg>` → path of the first `package:` line (base.apk).
pub fn parse_pm_path(out: &str) -> Option<String> {
    out.lines()
        .find_map(|l| l.trim().strip_prefix("package:"))
        .map(|p| p.trim().to_string())
}

/// `aapt dump badging` → `application-label:'…'`.
pub fn parse_badging_label(out: &str) -> Option<String> {
    out.lines().find_map(|l| {
        let rest = l.strip_prefix("application-label:'")?;
        let end = rest.rfind('\'')?;
        let label = clean_label(&rest[..end]);
        (!label.is_empty()).then_some(label)
    })
}

/// Undo aapt's escapes (`\n`, `\t`, `\'`, `\"`, `\\`) and fold whitespace runs into one space,
/// so multi-line app names fit a single row.
pub fn clean_label(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        match (c, chars.clone().next()) {
            ('\\', Some('n' | 't' | 'r')) => {
                chars.next();
                out.push(' ');
            }
            ('\\', Some(e @ ('\'' | '"' | '\\'))) => {
                chars.next();
                out.push(e);
            }
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Last dot-separated segment of the package name.
pub fn label_fallback(pkg: &str) -> String {
    pkg.rsplit('.').next().unwrap_or(pkg).to_string()
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LaunchResult {
    Success,
    BroughtToFront,
    NeedsOverlayPermission,
    Failed(String),
}

/// `termux-am` can't reach termux-app's am socket server (not running / disabled).
pub fn socket_unavailable(out: &str) -> bool {
    out.contains("Could not connect to socket") || out.contains("TermuxAm server is not enabled")
}

/// Classify `termux-am start` output (its exit code is always 0).
pub fn classify_launch(out: &str) -> LaunchResult {
    if out.contains("Display over other apps") {
        return LaunchResult::NeedsOverlayPermission;
    }
    if let Some(l) = out
        .lines()
        .find(|l| l.starts_with("Error") || l.contains("Exception") || l.contains("Error Code:"))
    {
        return LaunchResult::Failed(l.trim().to_string());
    }
    if out.contains("brought to the front") {
        return LaunchResult::BroughtToFront;
    }
    if out.contains("Starting: Intent") {
        return LaunchResult::Success;
    }
    let first = out.lines().find(|l| !l.trim().is_empty()).unwrap_or("no output");
    LaunchResult::Failed(first.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fx(name: &str) -> String {
        std::fs::read_to_string(format!("tests/fixtures/{name}")).unwrap()
    }

    #[test]
    fn query_activities() {
        let apps = parse_query_activities(&fx("query-activities.txt"));
        assert_eq!(apps.len(), 96);
        assert!(apps.contains(&(
            "com.android.chrome".into(),
            "com.android.chrome/com.google.android.apps.chrome.Main".into()
        )));
    }

    #[test]
    fn query_activities_first_per_package() {
        let s = "    a.b/a.b.One\n    a.b/a.b.Two\n    c.d/c.d.Main\n";
        let apps = parse_query_activities(s);
        assert_eq!(apps, vec![("a.b".into(), "a.b/a.b.One".into()), ("c.d".into(), "c.d/c.d.Main".into())]);
    }

    #[test]
    fn pm_path_base() {
        let p = parse_pm_path(&fx("pm-path-chrome.txt")).unwrap();
        assert!(p.ends_with("/base.apk"));
    }

    #[test]
    fn badging_label() {
        assert_eq!(parse_badging_label(&fx("badging-chrome.txt")).as_deref(), Some("Chrome"));
        assert_eq!(parse_badging_label("garbage"), None);
        assert_eq!(parse_badging_label("application-label:'Memory\\nHint'").as_deref(), Some("Memory Hint"));
    }

    #[test]
    fn cleans_label_escapes() {
        assert_eq!(clean_label("Memory\\nHint"), "Memory Hint");
        assert_eq!(clean_label(" A\\t\\n B "), "A B");
        assert_eq!(clean_label("Rock \\'n\\' Roll \\\\ x"), "Rock 'n' Roll \\ x");
    }

    #[test]
    fn fallback() {
        assert_eq!(label_fallback("com.example.foo"), "foo");
        assert_eq!(label_fallback("foo"), "foo");
    }

    #[test]
    fn classify() {
        assert_eq!(classify_launch(&fx("am-first.txt")), LaunchResult::Success);
        assert_eq!(classify_launch(&fx("am-front.synthetic.txt")), LaunchResult::BroughtToFront);
        assert_eq!(classify_launch(&fx("am-overlay.synthetic.txt")), LaunchResult::NeedsOverlayPermission);
        assert!(socket_unavailable("Could not connect to socket: No such file or directory\n"));
        assert!(!socket_unavailable(&fx("am-first.txt")));
        assert!(matches!(classify_launch(&fx("am-missing.txt")), LaunchResult::Failed(m) if m.contains("does not exist")));
        assert!(matches!(classify_launch(""), LaunchResult::Failed(_)));
    }
}
