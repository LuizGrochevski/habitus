use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Terminal;
use rusqlite::Connection;
use std::io;

use crate::db;
use crate::streak;

struct HabitSummary {
    name: String,
    streak: i32,
    record: i32,
    grid_text: String,
}

fn load_summaries(conn: &Connection) -> Result<Vec<HabitSummary>> {
    let today = chrono::Local::now().date_naive();
    let mut summaries = Vec::new();

    for h in db::list_habits(conn)? {
        let checkins = db::checkins_for(conn, &h.name)?;
        let target = db::daily_target(conn, &h.name)?;
        let qualifying = streak::qualifying_days(&checkins, target);

        summaries.push(HabitSummary {
            name: h.name,
            streak: streak::current_streak(&qualifying, today),
            record: streak::longest_streak(&qualifying),
            grid_text: streak::grid(&qualifying, today, 28),
        });
    }

    Ok(summaries)
}

enum View {
    List,
    Detail,
}

pub fn run(conn: &Connection) -> Result<()> {
    let summaries = load_summaries(conn)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut list_state = ListState::default();
    if !summaries.is_empty() {
        list_state.select(Some(0));
    }

    let result = event_loop(&mut terminal, &summaries, &mut list_state);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    summaries: &[HabitSummary],
    list_state: &mut ListState,
) -> Result<()> {
    let mut view = View::List;

    loop {
        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(3), Constraint::Length(3)])
                .split(frame.area());

            match view {
                View::List => {
                    let items: Vec<ListItem> = summaries
                        .iter()
                        .map(|s| {
                            ListItem::new(format!(
                                "{:20} streak: {:3}   recorde: {:3}",
                                s.name, s.streak, s.record
                            ))
                        })
                        .collect();

                    let list = List::new(items)
                        .block(Block::default().borders(Borders::ALL).title("habitus — hábitos"))
                        .highlight_style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Green))
                        .highlight_symbol("▶ ");

                    frame.render_stateful_widget(list, chunks[0], list_state);

                    let help = Paragraph::new("↑/↓ navegar   Enter ver grade   q / Esc sair")
                        .block(Block::default().borders(Borders::ALL));
                    frame.render_widget(help, chunks[1]);
                }
                View::Detail => {
                    let selected = list_state.selected().unwrap_or(0);
                    let summary = &summaries[selected];

                    let text = format!(
                        "Streak atual: {} dia(s)\nRecorde: {} dia(s)\n\nÚltimos 28 dias:\n{}",
                        summary.streak, summary.record, summary.grid_text
                    );

                    let detail = Paragraph::new(text).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!("habitus — {}", summary.name)),
                    );
                    frame.render_widget(detail, chunks[0]);

                    let help = Paragraph::new("Esc / Backspace voltar   q sair")
                        .block(Block::default().borders(Borders::ALL));
                    frame.render_widget(help, chunks[1]);
                }
            }
        })?;

        if event::poll(std::time::Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                match (&view, key.code) {
                    (_, KeyCode::Char('q')) => break,
                    (View::List, KeyCode::Esc) => break,
                    (View::List, KeyCode::Down) => select_next(list_state, summaries.len()),
                    (View::List, KeyCode::Up) => select_prev(list_state, summaries.len()),
                    (View::List, KeyCode::Enter) => {
                        if !summaries.is_empty() {
                            view = View::Detail;
                        }
                    }
                    (View::Detail, KeyCode::Esc) | (View::Detail, KeyCode::Backspace) => {
                        view = View::List;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn select_next(state: &mut ListState, len: usize) {
    if len == 0 {
        return;
    }
    let next = match state.selected() {
        Some(i) => (i + 1) % len,
        None => 0,
    };
    state.select(Some(next));
}

fn select_prev(state: &mut ListState, len: usize) {
    if len == 0 {
        return;
    }
    let prev = match state.selected() {
        Some(0) | None => len - 1,
        Some(i) => i - 1,
    };
    state.select(Some(prev));
}
