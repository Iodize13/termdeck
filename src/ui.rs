use crate::app::{Action, App, MAX_HOME_ROWS, Page};
use crate::icons::icon_for;
use crate::store::{AppEntry, ButtonStyle};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

const ROW_H: u16 = 3;
const PIN_W: u16 = 4;
// Terminal palette colors only (no backgrounds), so termdeck follows the terminal's theme.
const BORDER: Color = Color::Blue;
const ACCENT: Color = Color::Yellow;
const MUTED: Color = Color::DarkGray;
/// List row fills, cycled; Yellow (ACCENT) is kept for the selection.
const FILLS: [Color; 5] = [Color::Blue, Color::Magenta, Color::Cyan, Color::Green, Color::Red];
// Nerd Font star / star-outline (nf-fa-star, nf-fa-star_o).
const STAR_ON: &str = "\u{f005}";
const STAR_OFF: &str = "\u{f006}";
// Powerline-extra diagonals (nf-ple-*): forward-slash line, lower-right and upper-left triangles.
const SLASH: &str = "\u{e0bb}";
const LOWER_RIGHT: &str = "\u{e0ba}";
const UPPER_LEFT: &str = "\u{e0bc}";
const GEAR: &str = "\u{f013}";
const WARN: &str = "\u{f071}";

pub fn grid_columns(width: u16) -> u16 {
    (width / 20).max(2)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut t: String = s.chars().take(max - 1).collect();
        t.push('…');
        t
    }
}

/// Bordered block with `line` on its middle inner row. Selected blocks get a thick accent border.
/// Returns the inner area.
fn boxed(f: &mut Frame, area: Rect, line: Line, sel: bool, align: Alignment) -> Rect {
    if area.width < 2 || area.height < 2 {
        return Rect::default();
    }
    let (color, kind) = if sel { (ACCENT, BorderType::Thick) } else { (BORDER, BorderType::Plain) };
    let block = Block::default().borders(Borders::ALL).border_type(kind).border_style(Style::default().fg(color));
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.height > 0 {
        let mid = Rect { y: inner.y + inner.height.saturating_sub(1) / 2, height: 1, ..inner };
        f.render_widget(Paragraph::new(line).alignment(align), mid);
    }
    inner
}

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = f.area();
    if area.height < 6 || area.width < 8 {
        return;
    }
    let tabs = Rect { height: 1, ..area };
    let status = Rect { y: area.y + area.height - 1, height: 1, ..area };
    let prompt = Rect { y: status.y - 3, height: 3, ..area };
    let content = Rect { y: area.y + 1, height: area.height - 5, ..area };

    // The selection space depends on how many pins fit, so size the grid before `items()`.
    let cols = grid_columns(content.width);
    app.grid_cap = (cols * grid_max_rows(app, content.height)) as usize;

    let items: Vec<AppEntry> = app.items().into_iter().cloned().collect();
    draw_tabs(f, app, tabs);
    if app.page == Page::Settings {
        draw_settings(f, app, content);
    } else if app.searching() {
        draw_search(f, app, content, &items);
    } else {
        draw_page(f, app, content, &items);
    }
    draw_prompt(f, app, prompt);
    draw_status(f, app, status);
}

/// Grid rows a page may use: Home has a configured count, Apps leaves room for the Recent list.
/// One line is kept free for the "not shown" warning.
fn grid_max_rows(app: &App, height: u16) -> u16 {
    let fit = height.saturating_sub(1) / ROW_H;
    match app.page {
        Page::Home => fit.min(app.config.home_rows),
        Page::Apps => (height.saturating_sub(ROW_H + 2) / ROW_H).max(1).min(fit),
        Page::Settings => 0,
    }
}

fn draw_tabs(f: &mut Frame, app: &mut App, area: Rect) {
    let mut x = area.x;
    for page in Page::ALL {
        let text = format!(" {} ", page.title());
        let w = (text.chars().count() as u16).min(area.x + area.width - x);
        let style = if page == app.page {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(MUTED)
        };
        let rect = Rect { x, width: w, ..area };
        f.render_widget(Paragraph::new(Span::styled(text, style)), rect);
        app.hits.push((rect, Action::Goto(page)));
        x += w;
    }
    if matches!(app.page, Page::Apps | Page::Settings) {
        // Gear only when the full label would collide with the tabs.
        let room = area.x + area.width - x;
        let full = format!(" {GEAR} Settings ");
        let text = if full.chars().count() as u16 + 1 <= room { full } else { format!(" {GEAR} ") };
        let w = (text.chars().count() as u16).min(room);
        let style = if app.page == Page::Settings {
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
        } else {
            Style::default().fg(MUTED)
        };
        let rect = Rect { x: area.x + area.width - w, width: w, ..area };
        f.render_widget(Paragraph::new(Span::styled(text, style)), rect);
        app.hits.push((rect, Action::Goto(Page::Settings)));
    }
}

fn draw_settings(f: &mut Frame, app: &mut App, area: Rect) {
    app.visible_rows = 0;
    let rows = app.config.home_rows;
    let row_cells = vec![
        ("-".to_string(), Some(Action::AdjustHomeRows(-1)), rows > 1),
        (rows.to_string(), None, false),
        ("+".to_string(), Some(Action::AdjustHomeRows(1)), rows < MAX_HOME_ROWS),
    ];
    let style_cells = |cur: ButtonStyle, set: fn(ButtonStyle) -> Action| {
        [ButtonStyle::Outline, ButtonStyle::Filled]
            .into_iter()
            .map(|s| (format!("{s:?}"), Some(set(s)), s == cur))
            .collect::<Vec<_>>()
    };
    let sections = [
        ("Home rows", row_cells),
        ("Pinned style", style_cells(app.config.pinned_style, Action::SetPinnedStyle)),
        ("List style", style_cells(app.config.list_style, Action::SetListStyle)),
    ];
    let mut y = area.y + 1;
    for (title, cells) in sections {
        if y + 1 + ROW_H > area.y + area.height {
            return;
        }
        f.render_widget(
            Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
            Rect { y, height: 1, ..area },
        );
        settings_row(f, app, Rect { y: y + 1, height: ROW_H, ..area }, cells);
        y += 1 + ROW_H + 1;
    }
    let note = format!("Saved to {}", app.paths.config.display());
    f.render_widget(
        Paragraph::new(note).style(Style::default().fg(MUTED)).wrap(ratatui::widgets::Wrap { trim: true }),
        Rect { y, height: (area.y + area.height).saturating_sub(y), ..area },
    );
}

/// A row of equal boxes. Each cell is (text, tap action, highlighted); a cell without an action
/// shows a value, a highlighted one with an action is the current choice.
fn settings_row(f: &mut Frame, app: &mut App, row: Rect, cells: Vec<(String, Option<Action>, bool)>) {
    let n = cells.len() as u16;
    let bw = row.width / n;
    for (i, (text, action, hl)) in cells.into_iter().enumerate() {
        let i = i as u16;
        let rect = Rect { x: row.x + bw * i, width: if i == n - 1 { row.width - bw * i } else { bw.saturating_sub(1) }, ..row };
        let style = match (&action, hl) {
            (None, _) => Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            (Some(Action::AdjustHomeRows(_)), true) => Style::default().add_modifier(Modifier::BOLD),
            (Some(Action::AdjustHomeRows(_)), false) => Style::default().fg(MUTED),
            (Some(_), true) => Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            (Some(_), false) => Style::default(),
        };
        let current = hl && !matches!(action, None | Some(Action::AdjustHomeRows(_)));
        boxed(f, rect, Line::styled(text, style), current, Alignment::Center);
        if let Some(a) = action {
            app.hits.push((rect, a));
        }
    }
}

fn draw_page(f: &mut Frame, app: &mut App, area: Rect, items: &[AppEntry]) {
    let pinned_n = app.pinned().len();
    let mut grid_h = draw_grid(f, app, area, &items[..pinned_n]);
    // Home stays clean; pinning there already reports when the grid is full.
    let hidden = if app.page == Page::Apps { app.pinned_hidden() } else { 0 };
    if hidden > 0 && grid_h < area.height {
        let text = format!("{WARN} {hidden} pinned not shown");
        f.render_widget(
            Paragraph::new(truncate(&text, area.width as usize)).style(Style::default().fg(ACCENT)),
            Rect { y: area.y + grid_h, height: 1, ..area },
        );
        grid_h += 1;
    }
    let rest = Rect { y: area.y + grid_h, height: area.height - grid_h, ..area };
    match app.page {
        Page::Home => {
            app.visible_rows = 0;
            match &app.art {
                Some(art) => draw_art(f, art, rest),
                None => draw_katana(f, rest, app.started.elapsed().as_secs_f32()),
            }
        }
        Page::Settings => {}
        Page::Apps => {
            let recent = &items[pinned_n..];
            if rest.height < 2 || recent.is_empty() {
                app.visible_rows = 0;
                return;
            }
            f.render_widget(
                Paragraph::new("Recent").style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
                Rect { height: 1, ..rest },
            );
            let rows = Rect { y: rest.y + 1, height: rest.height - 1, ..rest };
            draw_list(f, app, rows, recent, pinned_n);
        }
    }
}

/// Grid of pinned-app buttons at the top of `area`; returns the height used.
fn draw_grid(f: &mut Frame, app: &mut App, area: Rect, pinned: &[AppEntry]) -> u16 {
    let cols = grid_columns(area.width);
    if pinned.is_empty() {
        let r = Rect { height: ROW_H.min(area.height), ..area };
        f.render_widget(
            Paragraph::new(format!("No pinned apps yet.\nSearch, then tap {STAR_OFF} to pin here."))
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center),
            r,
        );
        return r.height;
    }
    let bw = area.width / cols;
    for (i, e) in pinned.iter().enumerate() {
        let (c, r) = (i as u16 % cols, i as u16 / cols);
        let w = if c == cols - 1 { area.width - bw * c } else { bw };
        // 1 column gap between buttons, except on the last column.
        let rect = Rect {
            x: area.x + bw * c,
            y: area.y + r * ROW_H,
            width: if c == cols - 1 { w } else { w.saturating_sub(1) },
            height: ROW_H,
        };
        let sel = app.selected == i;
        let style = app.config.pinned_style;
        let color = match (sel, style) {
            (true, _) => ACCENT,
            (false, ButtonStyle::Outline) => BORDER,
            (false, ButtonStyle::Filled) => FILLS[i % FILLS.len()],
        };
        let inner = slanted(f, rect, style, color);
        let text = content_style(app, style, color, sel.then_some(ACCENT));
        // Bold only off a fill: some terminals render bold as the bright color, which on a fill
        // would no longer match the computed readable color.
        let text = if style == ButtonStyle::Outline { text.add_modifier(Modifier::BOLD) } else { text };
        f.render_widget(Paragraph::new(icon_label(e, inner.width, text)).style(text).alignment(Alignment::Center), inner);
        app.hits.push((rect, Action::Launch(e.pkg.clone())));
    }
    (pinned.len() as u16).div_ceil(cols) * ROW_H
}

/// Slanted (parallelogram) button, 3 rows, each row shifted one column left of the one above.
/// Nerd Font powerline glyphs make the sides straight: a cell's diagonal matches the one-column
/// shift per row. Returns the middle row's content area.
///   Outline:  ▔▔▔▔▔▔      Filled:  ██████
///             X abc               X abc 
///            ▁▁▁▁▁▁               ██████
fn slanted(f: &mut Frame, rect: Rect, style: ButtonStyle, color: Color) -> Rect {
    let (x, y, w) = (rect.x, rect.y, rect.width);
    if w < 6 || rect.height < 3 {
        return Rect::default();
    }
    let row_w = w - 2;
    let line = Style::default().fg(color);
    let buf = f.buffer_mut();
    for r in 0..3u16 {
        let (start, end) = (x + 2 - r, x + 2 - r + row_w - 1);
        let (left, right) = match style {
            ButtonStyle::Outline => (SLASH, SLASH),
            ButtonStyle::Filled => (LOWER_RIGHT, UPPER_LEFT),
        };
        buf[(start, y + r)].set_symbol(left).set_style(line);
        buf[(end, y + r)].set_symbol(right).set_style(line);
        for c in start + 1..end {
            let cell = &mut buf[(c, y + r)];
            match (style, r) {
                (ButtonStyle::Filled, _) => cell.set_symbol(" ").set_style(Style::default().bg(color)),
                (ButtonStyle::Outline, 0) => cell.set_symbol("▔").set_style(line),
                (ButtonStyle::Outline, 2) => cell.set_symbol("▁").set_style(line),
                _ => cell,
            };
        }
    }
    Rect { x: x + 2, y: y + 1, width: row_w - 2, height: 1 }
}

/// Content style inside a `slanted` button: on a fill, the readable text color; on an outline,
/// `outline_fg` (or the terminal default).
fn content_style(app: &App, style: ButtonStyle, color: Color, outline_fg: Option<Color>) -> Style {
    match style {
        ButtonStyle::Filled => Style::default().bg(color).fg(text_on(&app.palette, color)),
        ButtonStyle::Outline => outline_fg.map_or(Style::default(), |c| Style::default().fg(c)),
    }
}

/// Black (color0) or white (color15), whichever has the higher WCAG contrast on `fill`. Needs the
/// palette's RGB; without it, white.
fn text_on(palette: &[Option<(u8, u8, u8)>; 16], fill: Color) -> Color {
    let idx = match fill {
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        _ => return Color::White,
    };
    let lum = |(r, g, b): (u8, u8, u8)| {
        let lin = |c: u8| {
            let c = c as f32 / 255.0;
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
    };
    let contrast = |a: f32, b: f32| (a.max(b) + 0.05) / (a.min(b) + 0.05);
    let Some(bg) = palette[idx].map(lum) else { return Color::White };
    let black = palette[0].map_or(0.0, lum);
    let white = palette[15].map_or(1.0, lum);
    if contrast(bg, black) > contrast(bg, white) { Color::Black } else { Color::White }
}

/// "<icon> <label>", truncated to `width`.
fn icon_label(e: &AppEntry, width: u16, style: Style) -> Line<'static> {
    let icon = icon_for(&e.pkg, &e.label);
    if width < 5 {
        return Line::styled(icon, style);
    }
    let label = truncate(&e.label, (width as usize).saturating_sub(2));
    Line::styled(format!("{icon} {label}"), style)
}

/// Art centered in `area` as one block (lines keep their relative alignment); hidden if it doesn't fit.
fn draw_art(f: &mut Frame, art: &str, area: Rect) {
    let lines: Vec<Line> = art.lines().map(|l| Line::styled(l.to_string(), Style::default().fg(BORDER))).collect();
    let w = lines.iter().map(Line::width).max().unwrap_or(0) as u16;
    let h = lines.len() as u16;
    if w > area.width || h > area.height {
        return;
    }
    let rect = Rect {
        x: area.x + (area.width - w) / 2,
        y: area.y + (area.height - h) / 2,
        width: w,
        height: h,
    };
    f.render_widget(Paragraph::new(lines), rect);
}

/// A katana leaning 45° off the vertical axis and turning about it (its tip traces a cone), seen
/// from slightly above, sized to fit `area`. `t` is seconds since start.
fn draw_katana(f: &mut Frame, area: Rect, t: f32) {
    const TURN_SECS: f32 = 4.0;
    const TILT: f32 = 0.3; // camera pitch, radians
    const LEAN: f32 = std::f32::consts::FRAC_PI_4;
    // Fit the cone: half the sword sweeps ±len/2·sin(LEAN) (doubled for cell aspect) plus the tsuba.
    let (lsin, lcos) = LEAN.sin_cos();
    let by_width = (area.width as f32 / 2.0 - 2.0) / lsin;
    let len = (area.height as f32 - 3.0).min(20.0).min(by_width);
    if len < 6.0 || area.width < 8 {
        return;
    }
    let (sin, cos) = (t / TURN_SECS * std::f32::consts::TAU).sin_cos();
    let (tsin, tcos) = TILT.sin_cos();
    let (cx, cy) = ((area.x + area.width / 2) as f32, (area.y + area.height / 2) as f32);

    // Model space: y up the sword, x across the tsuba, z towards the blade's curve.
    // Returns screen column, row and depth (larger = nearer).
    let project = |x: f32, y: f32, z: f32| {
        let (x, y) = (x * lcos + y * lsin, y * lcos - x * lsin); // lean
        let (x, z) = (x * cos + z * sin, z * cos - x * sin); // spin about y
        let (y, z) = (y * tcos - z * tsin, y * tsin + z * tcos); // pitch about x
        // Cells are about twice as tall as wide, so x distances are doubled.
        (cx + x * 2.0, cy - y, z)
    };
    // Line glyph for an on-screen direction (x right, y up, in cells), folded to [0, π): │ ╱ ─ ╲.
    let glyph = |x: f32, y: f32, thick: bool| {
        let angle = x.atan2(y).rem_euclid(std::f32::consts::PI);
        let i = ((angle / std::f32::consts::FRAC_PI_4).round() as usize) % 4;
        if thick { ["┃", "╱", "━", "╲"][i] } else { ["│", "╱", "─", "╲"][i] }
    };

    let (pommel, tip) = (-len / 2.0, len / 2.0);
    let guard_at = pommel + len * 0.3;
    let sori = len * 0.08; // curve depth at the tip
    let curve = |y: f32| sori * ((y - guard_at) / (tip - guard_at)).powi(2);
    // The blade's flats face ±x; seen flat-on (|sin| near 1) it looks wide and shows its curve.
    let flat_on = sin.abs() > 0.5;

    let mut cells: Vec<(f32, f32, f32, &str, Color)> = Vec::new();
    let mut y = pommel;
    while y <= tip {
        if y < guard_at {
            let (sx, sy, d) = project(0.0, y, 0.0);
            cells.push((d, sx, sy, "▓", BORDER));
        } else if y >= guard_at + 0.75 {
            // (Starts a bit above the tsuba so it doesn't land on the tsuba's row.)
            let (sx, sy, d) = project(0.0, y, curve(y));
            let (nx, ny, _) = project(0.0, y + 0.5, curve(y + 0.5));
            cells.push((d, sx, sy, glyph(nx - sx, sy - ny, flat_on), Color::Gray));
        }
        y += 0.25;
    }
    // Oval tsuba (wider across x), filled, so it never vanishes edge-on but its width still turns.
    let mut x: f32 = -1.75;
    while x <= 1.75 {
        let mut z: f32 = -0.9;
        while z <= 0.9 {
            if (x / 1.75).powi(2) + (z / 0.9).powi(2) <= 1.0 {
                let (sx, sy, d) = project(x, guard_at, z);
                cells.push((d, sx, sy, "━", ACCENT));
            }
            z += 0.25;
        }
        x += 0.25;
    }

    // Far to near, so nearer parts cover farther ones.
    cells.sort_by(|a, b| a.0.total_cmp(&b.0));
    let buf = f.buffer_mut();
    for (_, sx, sy, sym, color) in cells {
        let (col, row) = (sx.round(), sy.round());
        if col < area.x as f32 || row < area.y as f32 || col >= (area.x + area.width) as f32 || row >= (area.y + area.height) as f32 {
            continue;
        }
        if let Some(cell) = buf.cell_mut((col as u16, row as u16)) {
            cell.set_symbol(sym).set_fg(color);
        }
    }
}

fn draw_search(f: &mut Frame, app: &mut App, area: Rect, items: &[AppEntry]) {
    if items.is_empty() {
        app.visible_rows = 0;
        f.render_widget(
            Paragraph::new("No matches").style(Style::default().fg(Color::Gray)).alignment(Alignment::Center),
            area,
        );
        return;
    }
    draw_list(f, app, area, items, 0);
}

/// Scrollable list of 3-row items. `base` is the index of `items[0]` in the selection space.
fn draw_list(f: &mut Frame, app: &mut App, area: Rect, items: &[AppEntry], base: usize) {
    let visible = (area.height / ROW_H) as usize;
    app.visible_rows = visible;
    if visible == 0 {
        return;
    }
    let max_scroll = items.len().saturating_sub(visible);
    app.scroll = app.scroll.min(max_scroll);
    if app.follow_selection && app.selected >= base {
        let rel = app.selected - base;
        if rel < app.scroll {
            app.scroll = rel;
        } else if rel >= app.scroll + visible {
            app.scroll = rel + 1 - visible;
        }
    }
    for (slot, e) in items.iter().skip(app.scroll).take(visible).enumerate() {
        let idx = base + app.scroll + slot;
        let rect = Rect { y: area.y + slot as u16 * ROW_H, height: ROW_H, ..area };
        let sel = app.selected == idx;
        let pinned = app.is_pinned(&e.pkg);

        let color = if sel { ACCENT } else { FILLS[idx % FILLS.len()] };
        let style = app.config.list_style;
        let inner = slanted(f, rect, style, color);
        // Icon and label share one color (no bold: it can render as the bright variant).
        let solid = content_style(app, style, color, Some(color));
        // Pin toggle on the right end of the bar; the whole height is the tap target.
        let pin_w = PIN_W.min(inner.width);
        let pin_rect = Rect { x: inner.x + inner.width - pin_w, width: rect.x + rect.width - (inner.x + inner.width - pin_w), ..rect };
        let label_rect = Rect { width: pin_rect.x - rect.x, ..rect };
        let text = Rect { width: inner.width - pin_w, ..inner };

        let avail = text.width as usize;
        let label = truncate(&e.label, avail.saturating_sub(3));
        let pkg_room = avail.saturating_sub(label.chars().count() + 5);
        let mut spans = vec![
            Span::styled(format!(" {} ", icon_for(&e.pkg, &e.label)), solid),
            Span::styled(label, if sel { solid.add_modifier(Modifier::UNDERLINED) } else { solid }),
        ];
        if pkg_room > 4 {
            spans.push(Span::styled(format!("  {}", truncate(&e.pkg, pkg_room)), solid));
        }
        f.render_widget(Paragraph::new(Line::from(spans)).style(solid), text);
        let glyph = if pinned { STAR_ON } else { STAR_OFF };
        let star = Rect { x: pin_rect.x, width: pin_w, ..inner };
        f.render_widget(Paragraph::new(glyph).style(solid).alignment(Alignment::Center), star);

        app.hits.push((pin_rect, Action::TogglePin(e.pkg.clone())));
        app.hits.push((label_rect, Action::Launch(e.pkg.clone())));
    }
}

fn draw_prompt(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Plain)
        .border_style(Style::default().fg(BORDER));
    let line = if app.query.is_empty() {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(ACCENT)),
            Span::styled("type to search…", Style::default().fg(MUTED)),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(ACCENT)),
            Span::raw(app.query.clone()),
            Span::styled("▌", Style::default().fg(ACCENT)),
        ])
    };
    f.render_widget(Paragraph::new(line).block(block), area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.entries.is_empty() && app.refreshing {
        "Indexing apps…".to_string()
    } else if app.status.is_empty() {
        "Tab page · ^P pin · ^R refresh · Esc quit".to_string()
    } else {
        app.status.clone()
    };
    let width = area.width as usize;
    f.render_widget(
        Paragraph::new(truncate(&text, width)).style(Style::default().fg(ACCENT)),
        area,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_from_width() {
        assert_eq!(grid_columns(45), 2);
        assert_eq!(grid_columns(30), 2);
        assert_eq!(grid_columns(100), 5);
    }

    #[test]
    fn truncates() {
        assert_eq!(truncate("abcdef", 4), "abc…");
        assert_eq!(truncate("abc", 4), "abc");
        assert_eq!(truncate("abc", 0), "");
    }
}
