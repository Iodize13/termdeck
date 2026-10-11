use crate::app::{Action, App, Page};
use crate::store::AppEntry;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

const ROW_H: u16 = 3;
const PIN_W: u16 = 4;
const HOME_ROWS: u16 = 3;
// Terminal palette colors only (no backgrounds), so termdeck follows the terminal's theme.
const BORDER: Color = Color::Blue;
const ACCENT: Color = Color::Yellow;
const MUTED: Color = Color::DarkGray;
// Nerd Font star / star-outline (nf-fa-star, nf-fa-star_o).
const STAR_ON: &str = "\u{f005}";
const STAR_OFF: &str = "\u{f006}";

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
    app.grid_cap = (cols * grid_max_rows(app.page, content.height)) as usize;

    let items: Vec<AppEntry> = app.items().into_iter().cloned().collect();
    draw_tabs(f, app, tabs);
    if app.searching() {
        draw_search(f, app, content, &items);
    } else {
        draw_page(f, app, content, &items);
    }
    draw_prompt(f, app, prompt);
    draw_status(f, app, status);
}

/// Grid rows a page may use: Home shows up to 3, Apps leaves room for the Recent list.
fn grid_max_rows(page: Page, height: u16) -> u16 {
    let fit = height / ROW_H;
    match page {
        Page::Home => fit.min(HOME_ROWS),
        Page::Apps => (height.saturating_sub(ROW_H + 1) / ROW_H).max(1).min(fit),
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
}

fn draw_page(f: &mut Frame, app: &mut App, area: Rect, items: &[AppEntry]) {
    let pinned_n = app.pinned().len();
    let grid_h = draw_grid(f, app, area, &items[..pinned_n]);
    let rest = Rect { y: area.y + grid_h, height: area.height - grid_h, ..area };
    match app.page {
        Page::Home => {
            app.visible_rows = 0;
            draw_art(f, &app.art, rest);
        }
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
        let style = Style::default().add_modifier(Modifier::BOLD);
        let style = if sel { style.fg(ACCENT) } else { style };
        let label = truncate(&e.label, (rect.width as usize).saturating_sub(2));
        boxed(f, rect, Line::styled(label, style), sel, Alignment::Center);
        app.hits.push((rect, Action::Launch(e.pkg.clone())));
    }
    (pinned.len() as u16).div_ceil(cols) * ROW_H
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

        // Border + 1 space of padding on the left; the pin column is on the right.
        let avail = (rect.width as usize).saturating_sub(3 + PIN_W as usize);
        let label = truncate(&e.label, avail);
        let pkg_room = avail.saturating_sub(label.chars().count() + 2);
        let label_style = Style::default().add_modifier(Modifier::BOLD);
        let mut spans = vec![
            Span::raw(" "),
            Span::styled(label, if sel { label_style.fg(ACCENT) } else { label_style }),
        ];
        if pkg_room > 4 {
            spans.push(Span::styled(format!("  {}", truncate(&e.pkg, pkg_room)), Style::default().fg(MUTED)));
        }
        let inner = boxed(f, rect, Line::from(spans), sel, Alignment::Left);

        // Pin toggle: the right end of the same box, border included so it's easy to tap.
        let pin_w = PIN_W.min(inner.width);
        let pin_rect = Rect { x: rect.x + rect.width - pin_w - 1, width: pin_w + 1, ..rect };
        let label_rect = Rect { width: rect.width - pin_rect.width, ..rect };
        let (color, glyph) = if pinned { (ACCENT, STAR_ON) } else { (MUTED, STAR_OFF) };
        let star = Rect { x: pin_rect.x, y: inner.y, width: pin_w, height: 1 };
        f.render_widget(Paragraph::new(Span::styled(glyph, Style::default().fg(color))).alignment(Alignment::Center), star);

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
