mod app;
mod index;
mod notes;
mod parse;
mod run;
mod icons;
mod store;
mod ui;

use app::{App, Page};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::execute;
use std::io::stdout;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let mut terminal = ratatui::init();
    execute!(stdout(), EnableMouseCapture)?;
    let result = run(&mut terminal);
    let _ = execute!(stdout(), DisableMouseCapture);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal) -> std::io::Result<()> {
    let mut app = App::new(store::Paths::from_env());
    while !app.should_quit {
        app.poll_index();
        terminal.draw(|f| ui::draw(f, &mut app))?;
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        match event::read()? {
            Event::Key(k) if k.kind != KeyEventKind::Release => {
                let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
                let settings = app.page == Page::Settings;
                match k.code {
                    KeyCode::Char('c') if ctrl => app.should_quit = true,
                    KeyCode::Char('p') if ctrl => app.toggle_pin_selected(),
                    KeyCode::Char('r') if ctrl => app.refresh(),
                    KeyCode::Char('n') if ctrl && !settings => app.toggle_note_mode(),
                    KeyCode::Tab => app.cycle_page(1),
                    KeyCode::BackTab => app.cycle_page(-1),
                    KeyCode::Esc => {
                        if app.note_mode {
                            app.cancel_note();
                        } else if app.searching() {
                            app.clear_query();
                        } else if settings {
                            app.goto(Page::Apps);
                        } else if app.page != Page::Home {
                            app.goto(Page::Home);
                        } else {
                            app.should_quit = true;
                        }
                    }
                    // Settings has no search; +/- and arrows adjust Home rows.
                    KeyCode::Char('+' | '=') | KeyCode::Right | KeyCode::Up if settings => app.adjust_home_rows(1),
                    KeyCode::Char('-') | KeyCode::Left | KeyCode::Down if settings => app.adjust_home_rows(-1),
                    _ if settings => {}
                    KeyCode::Enter if app.note_mode => app.save_note(),
                    KeyCode::Backspace if app.note_mode && app.query.is_empty() => app.note_mode = false,
                    // With text in the prompt, Left/Right move the cursor instead of the selection.
                    KeyCode::Left if !app.query.is_empty() => app.move_cursor(-1),
                    KeyCode::Right if !app.query.is_empty() => app.move_cursor(1),
                    KeyCode::Home if !app.query.is_empty() => app.move_cursor(isize::MIN / 2),
                    KeyCode::End if !app.query.is_empty() => app.move_cursor(isize::MAX / 2),
                    KeyCode::Up | KeyCode::Down | KeyCode::Left | KeyCode::Right if app.note_mode => {}
                    KeyCode::Enter if app.page == Page::Notes && !app.searching() => app.edit_selected_note(),
                    KeyCode::Enter => app.launch_selected(),
                    KeyCode::Backspace => app.backspace(),
                    KeyCode::Up | KeyCode::Left => app.move_selection(-1),
                    KeyCode::Down | KeyCode::Right => app.move_selection(1),
                    KeyCode::Char(c) if !ctrl => app.type_char(c),
                    _ => {}
                }
            }
            Event::Mouse(m) => match m.kind {
                MouseEventKind::Down(MouseButton::Left) => app.click(m.column, m.row),
                MouseEventKind::ScrollUp => app.wheel(-1),
                MouseEventKind::ScrollDown => app.wheel(1),
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}
