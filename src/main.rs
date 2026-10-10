mod app;
mod index;
mod parse;
mod run;
mod store;
mod ui;

use app::App;
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
                match k.code {
                    KeyCode::Char('c') if ctrl => app.should_quit = true,
                    KeyCode::Char('p') if ctrl => app.toggle_pin_selected(),
                    KeyCode::Char('r') if ctrl => app.refresh(),
                    KeyCode::Esc => {
                        if app.searching() {
                            app.clear_query();
                        } else {
                            app.should_quit = true;
                        }
                    }
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
                MouseEventKind::ScrollUp => app.scroll_by(-1),
                MouseEventKind::ScrollDown => app.scroll_by(1),
                _ => {}
            },
            _ => {}
        }
    }
    Ok(())
}
