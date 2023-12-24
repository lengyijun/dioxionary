use crate::theme::THEME;
use crate::{query, supermemo::Deck};
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use itertools::Itertools;
use ratatui::style::Stylize;
use ratatui::{prelude::*, widgets::*};
use std::io;
use std::time::{Duration, Instant};

#[derive(Clone, Copy)]
enum AnswerStatus {
    Show,
    Hide,
}

#[derive(Clone, Copy)]
enum ExitCode {
    ManualExit,
    OutOfCard,
}

impl AnswerStatus {
    fn flip(self) -> Self {
        match self {
            AnswerStatus::Show => AnswerStatus::Hide,
            AnswerStatus::Hide => AnswerStatus::Show,
        }
    }
}

/// App holds the state of the application
struct App {
    question: String,
    answer: String,
    answer_status: AnswerStatus,
    spent_time: Option<Duration>,

    vertical_scroll_state: ScrollbarState,
    horizontal_scroll_state: ScrollbarState,
    vertical_scroll: usize,
    horizontal_scroll: usize,
}

impl App {
    fn toggle(&mut self) {
        self.answer_status = self.answer_status.flip();
    }

    fn get_answer(&self) -> &str {
        match self.answer_status {
            AnswerStatus::Show => &self.answer,
            AnswerStatus::Hide => "",
        }
    }
}

pub fn main() -> Result<()> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    match res {
        Ok(ExitCode::ManualExit) => {}
        Ok(ExitCode::OutOfCard) => {
            println!("All cards reviewed");
        }
        Err(err) => {
            eprintln!("{err:?}");
        }
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>) -> io::Result<ExitCode> {
    let mut deck = Deck::load();
    let Some(mut app) = next(&mut deck) else {
        return Ok(ExitCode::OutOfCard);
    };

    loop {
        terminal.draw(|f| ui(f, &mut app))?;
        let start = Instant::now();

        if let Event::Key(key) = event::read()? {
            match &app.answer_status {
                AnswerStatus::Show => match key.code {
                    KeyCode::Char('h') | KeyCode::Char('H') => {
                        let spent_time = app.spent_time.unwrap();
                        let q = if spent_time < Duration::from_secs(5) {
                            2
                        } else {
                            1
                        };
                        deck.update(app.question.to_owned(), q);

                        let Some(new_app) = next(&mut deck) else {
                            return Ok(ExitCode::OutOfCard);
                        };
                        app = new_app
                    }
                    KeyCode::Char('g') | KeyCode::Char('G') => {
                        let spent_time = app.spent_time.unwrap();
                        let q = if spent_time < Duration::from_secs(5) {
                            5
                        } else if spent_time < Duration::from_secs(15) {
                            4
                        } else {
                            3
                        };
                        deck.update(app.question.to_owned(), q);

                        let Some(new_app) = next(&mut deck) else {
                            return Ok(ExitCode::OutOfCard);
                        };
                        app = new_app
                    }
                    KeyCode::Char('f') | KeyCode::Char('F') => {
                        deck.update(app.question.to_owned(), 0);

                        let Some(new_app) = next(&mut deck) else {
                            return Ok(ExitCode::OutOfCard);
                        };
                        app = new_app
                    }
                    KeyCode::Char(' ') => app.toggle(),

                    KeyCode::Down => {
                        app.vertical_scroll = app.vertical_scroll.saturating_add(1);
                        app.vertical_scroll_state =
                            app.vertical_scroll_state.position(app.vertical_scroll);
                    }
                    KeyCode::Up => {
                        app.vertical_scroll = app.vertical_scroll.saturating_sub(1);
                        app.vertical_scroll_state =
                            app.vertical_scroll_state.position(app.vertical_scroll);
                    }
                    KeyCode::Left => {
                        app.horizontal_scroll = app.horizontal_scroll.saturating_sub(1);
                        app.horizontal_scroll_state =
                            app.horizontal_scroll_state.position(app.horizontal_scroll);
                    }
                    KeyCode::Right => {
                        app.horizontal_scroll = app.horizontal_scroll.saturating_add(1);
                        app.horizontal_scroll_state =
                            app.horizontal_scroll_state.position(app.horizontal_scroll);
                    }

                    KeyCode::Char('q') | KeyCode::Esc => return Ok(ExitCode::ManualExit),
                    _ => {}
                },
                AnswerStatus::Hide => match key.code {
                    KeyCode::Char(' ') => {
                        let end = Instant::now();
                        let duration = end - start;
                        if app.spent_time.is_none() {
                            app.spent_time = Some(duration);
                        }
                        app.toggle();
                    }
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(ExitCode::ManualExit),
                    _ => {}
                },
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // question
            Constraint::Min(1),    // answer
            Constraint::Length(1), // button
        ])
        .split(f.size());

    let question = Paragraph::new(app.question.as_str())
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(question, chunks[0]);

    let answer = Paragraph::new(app.get_answer())
        .alignment(Alignment::Left)
        .block(Block::default().borders(Borders::ALL))
        .scroll((app.vertical_scroll as u16, (app.horizontal_scroll as u16)));
    f.render_widget(answer, chunks[1]);
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        chunks[1],
        &mut app.vertical_scroll_state,
    );
    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::HorizontalBottom)
            .thumb_symbol("🬋")
            .end_symbol(None),
        chunks[1].inner(&Margin {
            vertical: 0,
            horizontal: 1,
        }),
        &mut app.horizontal_scroll_state,
    );

    let escape_keys = [("Q/Esc", "Quit")];
    let hide_keys = [("<Space>", "Show answer")];
    let show_keys = [("f", "Forget"), ("h", "Hard"), ("g", "Good")];

    let keys: &[(&str, &str)] = match app.answer_status {
        AnswerStatus::Show => &show_keys,
        AnswerStatus::Hide => &hide_keys,
    };

    let spans = escape_keys
        .iter()
        .flat_map(|(key, desc)| {
            let key = Span::styled(format!(" {key} "), THEME.key_binding.key);
            let desc = Span::styled(format!(" {desc} "), THEME.key_binding.description);
            [key, desc]
        })
        .collect_vec();
    let buttons = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Right)
        .fg(Color::Indexed(236))
        .bg(Color::Indexed(232));
    f.render_widget(buttons, chunks[2]);

    let spans = keys
        .iter()
        .flat_map(|(key, desc)| {
            let key = Span::styled(format!(" {key} "), THEME.key_binding.key);
            let desc = Span::styled(format!(" {desc} "), THEME.key_binding.description);
            [key, desc]
        })
        .collect_vec();
    let buttons = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .fg(Color::Indexed(236));
    f.render_widget(buttons, chunks[2]);
}

fn next(deck: &mut Deck) -> Option<App> {
    let Some(question) = deck.search_reviewable() else {
        return None;
    };
    if let Ok((_, answer)) = query(&question) {
        let answer = answer.trim();
        let (height, width) = get_width_and_height(answer);
        Some(App {
            question,
            answer: answer.to_owned(),
            answer_status: AnswerStatus::Hide,
            spent_time: None,
            vertical_scroll_state: ScrollbarState::new(height),
            horizontal_scroll_state: ScrollbarState::new(width),
            vertical_scroll: 0,
            horizontal_scroll: 0,
        })
    } else {
        deck.0.remove(&question);
        next(deck)
    }
}

fn get_width_and_height(s: &str) -> (usize, usize) {
    let v: Vec<_> = s.split("\n").collect();
    let height = v.len();
    let width = v.into_iter().fold(10usize, |res, x| Ord::max(res, x.len()));
    (height, width)
}
