use crate::app::{Action, App};
use crate::store::AppEntry;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

const ROW_H: u16 = 3;
const PIN_W: u16 = 6;
const BTN_BG: Color = Color::Rgb(36, 84, 160);
const BTN_FG: Color = Color::White;
const SEL_BG: Color = Color::Rgb(240, 180, 40);
const SEL_FG: Color = Color::Black;
const ROW_BG: Color = Color::Rgb(34, 38, 48);
const PIN_ON_BG: Color = Color::Rgb(200, 150, 30);
const PIN_OFF_BG: Color = Color::Rgb(62, 68, 84);

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

/// Filled block with the text on its middle row.
fn filled(f: &mut Frame, area: Rect, text: &str, style: Style, align: Alignment) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(Block::default().style(style), area);
    let mid = Rect { y: area.y + area.height / 2, height: 1, ..area };
    f.render_widget(Paragraph::new(text.to_string()).style(style).alignment(align), mid);
}

pub fn draw(f: &mut Frame, app: &mut App) {
    app.hits.clear();
    let area = f.area();
    if area.height < 5 || area.width < 8 {
        return;
    }
    let status = Rect { y: area.y + area.height - 1, height: 1, ..area };
    let prompt = Rect { y: status.y - 3, height: 3, ..area };
    let content = Rect { height: area.height - 4, ..area };

    let items: Vec<AppEntry> = app.items().into_iter().cloned().collect();
    if app.searching() {
        draw_search(f, app, content, &items);
    } else {
        draw_home(f, app, content, &items);
    }
    draw_prompt(f, app, prompt);
    draw_status(f, app, status);
}

fn draw_home(f: &mut Frame, app: &mut App, area: Rect, items: &[AppEntry]) {
    let pinned_n = app.pinned().len();
    let cols = grid_columns(area.width);
    let grid_rows = (pinned_n as u16).div_ceil(cols);
    let grid_h = if pinned_n == 0 { ROW_H } else { grid_rows * ROW_H }.min(area.height.saturating_sub(ROW_H));

    if pinned_n == 0 {
        let r = Rect { height: grid_h, ..area };
        f.render_widget(
            Paragraph::new("No pinned apps yet.\nType to search, tap [☆] to pin.")
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center),
            r,
        );
    } else {
        let bw = area.width / cols;
        for (i, e) in items.iter().take(pinned_n).enumerate() {
            let (c, r) = (i as u16 % cols, i as u16 / cols);
            if (r + 1) * ROW_H > grid_h {
                break;
            }
            let w = if c == cols - 1 { area.width - bw * c } else { bw };
            // 1 column gap between buttons, except on the last column.
            let rect = Rect {
                x: area.x + bw * c,
                y: area.y + r * ROW_H,
                width: if c == cols - 1 { w } else { w.saturating_sub(1) },
                height: ROW_H,
            };
            let sel = app.selected == i;
            let style = if sel {
                Style::default().bg(SEL_BG).fg(SEL_FG).add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(BTN_BG).fg(BTN_FG).add_modifier(Modifier::BOLD)
            };
            let label = truncate(&e.label, (rect.width as usize).saturating_sub(2));
            filled(f, rect, &label, style, Alignment::Center);
            app.hits.push((rect, Action::Launch(e.pkg.clone())));
        }
    }

    let list = Rect { y: area.y + grid_h, height: area.height - grid_h, ..area };
    if list.height < 2 {
        app.visible_rows = 0;
        return;
    }
    let recent = &items[pinned_n.min(items.len())..];
    if recent.is_empty() {
        app.visible_rows = 0;
        return;
    }
    f.render_widget(
        Paragraph::new("Recent").style(Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)),
        Rect { height: 1, ..list },
    );
    let rows = Rect { y: list.y + 1, height: list.height - 1, ..list };
    draw_list(f, app, rows, recent, pinned_n);
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
        let (bg, fg) = if sel { (SEL_BG, SEL_FG) } else { (ROW_BG, Color::White) };
        let label_rect = Rect { width: rect.width.saturating_sub(PIN_W), ..rect };
        let pin_rect = Rect { x: rect.x + label_rect.width, width: rect.width - label_rect.width, ..rect };

        f.render_widget(Block::default().style(Style::default().bg(bg)), label_rect);
        let avail = (label_rect.width as usize).saturating_sub(2);
        let label = truncate(&e.label, avail);
        let pkg_room = avail.saturating_sub(label.chars().count() + 2);
        let mut spans = vec![Span::styled(label, Style::default().fg(fg).add_modifier(Modifier::BOLD))];
        if pkg_room > 4 {
            spans.push(Span::styled(
                format!("  {}", truncate(&e.pkg, pkg_room)),
                Style::default().fg(if sel { Color::Rgb(60, 40, 0) } else { Color::DarkGray }),
            ));
        }
        let text_rect = Rect { x: label_rect.x + 1, width: label_rect.width.saturating_sub(1), y: rect.y + 1, height: 1 };
        f.render_widget(Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)), text_rect);

        let pinned = app.is_pinned(&e.pkg);
        let (pbg, glyph) = if pinned { (PIN_ON_BG, "[★]") } else { (PIN_OFF_BG, "[☆]") };
        filled(f, pin_rect, glyph, Style::default().bg(pbg).fg(if pinned { Color::Black } else { Color::White }), Alignment::Center);

        app.hits.push((pin_rect, Action::TogglePin(e.pkg.clone())));
        app.hits.push((label_rect, Action::Launch(e.pkg.clone())));
    }
}

fn draw_prompt(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default().borders(Borders::ALL).border_type(BorderType::Plain);
    let line = if app.query.is_empty() {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::styled("type to search…", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Cyan)),
            Span::raw(app.query.clone()),
            Span::styled("▌", Style::default().fg(Color::Cyan)),
        ])
    };
    f.render_widget(Paragraph::new(line).block(block), area);
}

fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let text = if app.entries.is_empty() && app.refreshing {
        "Indexing apps…".to_string()
    } else if app.status.is_empty() {
        "Ctrl+P pin · Ctrl+R refresh · Esc quit".to_string()
    } else {
        app.status.clone()
    };
    let width = area.width as usize;
    f.render_widget(
        Paragraph::new(truncate(&text, width)).style(Style::default().fg(Color::Yellow)),
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
