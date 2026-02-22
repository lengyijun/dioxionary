#![feature(let_chains)]

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use dioxionary::review_helper::AnswerStatus;
use dioxionary::review_helper::ExitCode;
use dioxionary::spaced_repetition::SpacedRepetiton;
use dioxionary::theme::THEME;
use itertools::Itertools;
use ratatui::{prelude::*, widgets::*};
use std::io;
use std::process::Command;
use urlencoding::encode;

/// App holds the state of the application
struct App {
    question: String,
    answer_status: AnswerStatus,
}

impl App {
    fn toggle(&mut self) {
        self.answer_status = self.answer_status.flip();
    }

    fn render_answer(&self, f: &mut Frame, area: Rect) {
        match self.answer_status {
            AnswerStatus::Show => {
                let url = format!("goldendict://{}", encode(&self.question));
                let _ = Command::new("xdg-open").arg(&url).status();
                let text = Text::from(url).alignment(Alignment::Center);
                f.render_widget(text, area);
            }
            AnswerStatus::Hide => {
                let text = Text::from("");
                f.render_widget(text, area);
            }
        }
    }
}

pub fn main() -> Result<()> {
    // setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut history: Vec<String> = Vec::new();

    let res = run_app(
        &mut terminal,
        dioxionary::fsrs::sqlite_history::SQLiteHistory::default(),
        &mut history,
    );

    // restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    match res {
        Ok(ExitCode::ManualExit) => {
            println!("{:?}", history);
        }
        Ok(ExitCode::OutOfCard) => {
            println!("{:?}", history);
            println!("All cards reviewed");
        }
        Err(err) => {
            eprintln!("{err:?}");
        }
    }

    Ok(())
}

fn run_app<B: Backend, T: SpacedRepetiton>(
    terminal: &mut Terminal<B>,
    mut spaced_repetition: T,
    history: &mut Vec<String>,
) -> Result<ExitCode> {
    let Some(mut app) = next(&mut spaced_repetition) else {
        return Ok(ExitCode::OutOfCard);
    };

    loop {
        if let Some(last_review) = history.last()
            && last_review == &app.question
        {
        } else {
            history.push(app.question.clone());
        }
        terminal.draw(|f| ui(f, &mut app))?;

        loop {
            if let Event::Key(key) = event::read()? {
                match &app.answer_status {
                    AnswerStatus::Show => match key.code {
                        KeyCode::Char('a') | KeyCode::Char('A') => {
                            spaced_repetition
                                .update(app.question.to_owned(), rs_fsrs::Rating::Again)?;

                            let Some(new_app) = next(&mut spaced_repetition) else {
                                return Ok(ExitCode::OutOfCard);
                            };
                            app = new_app;
                            break;
                        }
                        KeyCode::Char('h') | KeyCode::Char('H') => {
                            spaced_repetition
                                .update(app.question.to_owned(), rs_fsrs::Rating::Hard)?;

                            let Some(new_app) = next(&mut spaced_repetition) else {
                                return Ok(ExitCode::OutOfCard);
                            };
                            app = new_app;
                            break;
                        }
                        KeyCode::Char('g') | KeyCode::Char('G') => {
                            spaced_repetition
                                .update(app.question.to_owned(), rs_fsrs::Rating::Good)?;

                            let Some(new_app) = next(&mut spaced_repetition) else {
                                return Ok(ExitCode::OutOfCard);
                            };
                            app = new_app;
                            break;
                        }
                        KeyCode::Char('e') | KeyCode::Char('E') => {
                            spaced_repetition
                                .update(app.question.to_owned(), rs_fsrs::Rating::Easy)?;

                            let Some(new_app) = next(&mut spaced_repetition) else {
                                return Ok(ExitCode::OutOfCard);
                            };
                            app = new_app;
                            break;
                        }
                        KeyCode::Char(' ') => {
                            app.toggle();
                            break;
                        }

                        KeyCode::Char('j') | KeyCode::Down => {
                            break;
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            break;
                        }
                        KeyCode::Home => {
                            break;
                        }
                        KeyCode::End => {
                            break;
                        }
                        KeyCode::PageUp => {
                            break;
                        }
                        KeyCode::PageDown => {
                            break;
                        }
                        KeyCode::Left => {
                            break;
                        }
                        KeyCode::Right => {
                            break;
                        }

                        KeyCode::Char('q') | KeyCode::Esc => return Ok(ExitCode::ManualExit),
                        _ => {}
                    },
                    AnswerStatus::Hide => match key.code {
                        KeyCode::Char(' ') => {
                            app.toggle();
                            break;
                        }
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(ExitCode::ManualExit),
                        _ => {}
                    },
                }
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

    app.render_answer(f, chunks[1]);

    let escape_keys = [("Q/Esc", "Quit")];
    let hide_keys = [("<Space>", "Show answer")];
    let show_keys = [("a", "Again"), ("h", "Hard"), ("g", "Good"), ("e", "Easy")];
    let help_keys = [("j/k", "Up/Down"), ("Home/End", "Top/Bottom")];

    let keys: &[(&str, &str)] = match app.answer_status {
        AnswerStatus::Show => &show_keys,
        AnswerStatus::Hide => &hide_keys,
    };

    let spans = keys2span(&escape_keys);
    let buttons = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Right)
        .fg(Color::Indexed(236))
        .bg(Color::Indexed(232));
    f.render_widget(buttons, chunks[2]);

    let spans = keys2span(keys);
    let buttons = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Center)
        .fg(Color::Indexed(236));
    f.render_widget(buttons, chunks[2]);

    if app.answer_status == AnswerStatus::Show {
        let spans = keys2span(&help_keys);
        let buttons = Paragraph::new(Line::from(spans))
            .alignment(Alignment::Left)
            .fg(Color::Indexed(236));
        f.render_widget(buttons, chunks[2]);
    }
}

fn next<T>(spaced_repetition: &mut T) -> Option<App>
where
    T: SpacedRepetiton,
{
    let Ok(Some(question)) = spaced_repetition.next_to_review() else {
        return None;
    };
    Some(App {
        question,
        answer_status: AnswerStatus::Hide,
    })
}

fn keys2span<'a>(keys: &'a [(&str, &str)]) -> Vec<Span<'a>> {
    keys.iter()
        .flat_map(|(key, desc)| {
            let key = Span::styled(format!(" {key} "), THEME.key_binding.key);
            let desc = Span::styled(format!(" {desc} "), THEME.key_binding.description);
            [key, desc]
        })
        .collect_vec()
}
