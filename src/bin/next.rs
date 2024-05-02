#![feature(let_chains)]

use anyhow::Result;
use chrono::DateTime;
use chrono::Local;
use clap::Parser;
use dioxionary::fsrs::Deck;
use dioxionary::fsrs::MemoryStateWrapper;
use dioxionary::history;
use dioxionary::query_and_push_tty;
use dioxionary::spaced_repetition::SpacedRepetiton;
use dioxionary::stardict::NotFoundError;
use std::collections::HashSet;
use std::env;
use std::str::FromStr;
use walkdir::WalkDir;

#[derive(Parser, Debug)]
struct Args {
    suffix: Vec<String>,

    #[arg(long, default_value_t = false)]
    random: bool,

    // show answer
    #[arg(long, default_value_t = false)]
    answer: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let w = if args.random {
        let deck = Deck::default();
        match deck.next_to_review() {
            Ok(Some(s)) => s,
            _ => {
                eprintln!("all reviewed");
                return Err(NotFoundError.into());
            }
        }
    } else {
        foo()?
    };

    let w = w.to_lowercase();
    let w = if let Some(suffix) = args.suffix.first() {
        format!("{w}.{suffix}")
    } else {
        w
    };
    println!("{w}");
    eprintln!("{w}");

    if args.answer {
        query_and_push_tty(&w);
    }

    Ok(())
}

fn foo() -> Result<String> {
    let mut allowed_prefix = HashSet::from([
        'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r',
        's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    ]);
    let walk_dir = WalkDir::new(env::current_dir().unwrap()).max_depth(1);
    for x in walk_dir {
        let Ok(x) = x else { continue };
        let Some(x) = x.file_name().to_str() else {
            continue;
        };
        let Some(c) = x.chars().next() else { continue };
        let Some(c) = c.to_lowercase().next() else {
            continue;
        };
        allowed_prefix.remove(&c);
    }

    let conn = history::get_db().unwrap();
    let mut stmt =
        conn.prepare("SELECT word, stability, difficulty, interval, last_reviewed FROM fsrs")?;
    let person_iter = stmt.query_map([], |row| {
        let time: String = row.get(4)?;
        let sm = MemoryStateWrapper {
            stability: row.get(1)?,
            difficulty: row.get(2)?,
            interval: row.get(3)?,
            last_reviewed: DateTime::<Local>::from_str(&time).unwrap(),
        };
        let word: String = row.get(0)?;
        Ok((word.to_lowercase(), sm.next_review_time()))
    })?;
    let mut v: Vec<(String, _)> = person_iter.flatten().collect();
    v.sort_by(|(_, a), (_, b)| a.cmp(b));

    for (w, _) in v {
        if w.contains(' ') {
            continue;
        };
        let Some(c) = w.chars().next() else { continue };
        if allowed_prefix.contains(&c) {
            return Ok(w);
        }
    }

    Err(NotFoundError.into())
}
