//! Nerd Font glyphs for apps, guessed from package name and label. Font Awesome 4 codepoints
//! (nf-fa-*), which every Nerd Font carries.

/// (needles matched against lowercased pkg and label, glyph). First match wins, so specific
/// needles come before generic ones.
const ICONS: &[(&[&str], &str)] = &[
    (&["youtube"], "\u{f16a}"),
    (&["github"], "\u{f09b}"),
    (&["chrome"], "\u{f268}"),
    (&["firefox", "mozilla"], "\u{f269}"),
    (&["telegram"], "\u{f2c6}"),
    (&["spotify"], "\u{f1bc}"),
    (&["reddit"], "\u{f1a1}"),
    (&["twitter"], "\u{f099}"),
    (&["whatsapp"], "\u{f232}"),
    (&["instagram"], "\u{f16d}"),
    (&["facebook.katana"], "\u{f09a}"),
    (&["steam"], "\u{f1b6}"),
    (&["termux", "terminal", "console"], "\u{f120}"),
    (&["settings"], "\u{f013}"),
    (&["camera"], "\u{f030}"),
    (&["calendar"], "\u{f073}"),
    (&["gmail", "mail"], "\u{f0e0}"),
    (&["maps"], "\u{f041}"),
    (&["calculator"], "\u{f1ec}"),
    (&["gallery", "photos"], "\u{f03e}"),
    (&["music", "vlc", "player"], "\u{f001}"),
    (&["vending", "store"], "\u{f07a}"),
    (&["docs", "drive", "cloud"], "\u{f0c2}"),
    (&["keyboard"], "\u{f11c}"),
    (&["dialer", "phone"], "\u{f095}"),
    (&["contacts"], "\u{f2b9}"),
    (&["clock", "alarm"], "\u{f017}"),
    (&["files", "documentsui"], "\u{f07b}"),
    (&["authenticator", "1.1.1.1", "onedotone", "vpn"], "\u{f132}"),
    (&["classroom"], "\u{f19d}"),
    (&["grab", "uber", "bolt"], "\u{f1b9}"),
    (&["claude", "chatgpt", "openai", "gemini"], "\u{f0eb}"),
    (&["messenger", "orca", "messag", "naver.line", "discord", "chat"], "\u{f075}"),
    (&["fitness", "health", "boostcamp", "workout"], "\u{f21e}"),
    (&["browser", "duckduckgo", "brave", "opera"], "\u{f0ac}"),
];
const DEFAULT: &str = "\u{f17b}"; // android

pub fn icon_for(pkg: &str, label: &str) -> &'static str {
    let (pkg, label) = (pkg.to_lowercase(), label.to_lowercase());
    ICONS
        .iter()
        .find(|(needles, _)| needles.iter().any(|n| pkg.contains(n) || label.contains(n)))
        .map_or(DEFAULT, |(_, g)| g)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_pkg_or_label() {
        assert_eq!(icon_for("app.revanced.android.youtube", "YouTube Morphe"), "\u{f16a}");
        assert_eq!(icon_for("com.github.android", "GitHub"), "\u{f09b}");
        assert_eq!(icon_for("com.example.x", "Settings"), "\u{f013}");
        assert_eq!(icon_for("com.example.unknown", "Thing"), DEFAULT);
    }
}
