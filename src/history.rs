//! History query and addition using [sqlite](https://sqlite.org/index.html).
use anyhow::{Context, Result};
use chrono::Utc;
use dirs::cache_dir;
use prettytable::{Attr, Cell, Row, Table};
use rusqlite::Connection;
use std::fs::create_dir;
use std::path::PathBuf;

use crate::supermemo::{Deck, Sm};

/// Allowed diffculty level types of a word.
pub static ALLOWED_TYPES: [&str; 7] = ["CET4", "CET6", "TOEFL", "IELTS", "GMAT", "GRE", "SAT"];

/// Check and generate cache directory path.
fn check_cache() -> Result<PathBuf> {
    let mut path = cache_dir().with_context(|| "Couldn't find cache directory")?;
    path.push("dioxionary");
    if !path.exists() {
        create_dir(&path).with_context(|| format!("Failed to create directory {:?}", path))?;
    }
    path.push("dioxionary.db");
    Ok(path)
}

/// Add a looked up word to history.
pub fn add_history(word: String) -> Result<()> {
    let mut deck = Deck::load();
    match deck.0.entry(word) {
        std::collections::hash_map::Entry::Occupied(_) => {}
        std::collections::hash_map::Entry::Vacant(v) => {
            v.insert(Sm::default());
        }
    }
    deck.dump()
}

/// List sorted or not history of a word type or all types.
///
/// The output will be like:
/// txt
/// +------+------+-------+-------+------+-----+-----+
/// | CET4 | CET6 | TOEFL | IELTS | GMAT | GRE | SAT |
/// +------+------+-------+-------+------+-----+-----+
/// | 220  | 305  | 207   | 203   | 142  | 242 | 126 |
/// +------+------+-------+-------+------+-----+-----+
///
pub fn list_history(type_: Option<String>, sort: bool, table: bool, column: usize) -> Result<()> {
    let path = check_cache()?;

    let mut stmt = "SELECT WORD, DATE FROM HISTORY".to_string();

    if let Some(type_) = type_ {
        if ALLOWED_TYPES.contains(&type_.as_str()) {
            stmt.push_str(format!(" WHERE {} = 1", type_).as_str())
        }
    }

    let conn = Connection::open(path)?;

    let mut stmt = conn.prepare(&stmt)?;
    let word_iter = stmt.query_map([], |row| row.get(0) as rusqlite::Result<String>)?;

    let mut words: Vec<String> = word_iter.filter_map(|x| x.ok()).collect();

    if sort {
        words.sort();
    }

    if table {
        let mut table = Table::new();
        words.chunks(column).for_each(|x| {
            table.add_row(x.iter().map(|x| Cell::new(x)).collect());
        });
        table.printstd();
    } else {
        words.into_iter().for_each(|x| {
            println!("{}", x);
        });
    }

    Ok(())
}

/// Count the history.
pub fn count_history() -> Result<()> {
    let path = check_cache()?;

    let conn = Connection::open(path)?;

    let header: Row = ALLOWED_TYPES
        .into_iter()
        .map(|x| Cell::new(x).with_style(Attr::Bold))
        .collect();

    let mut table: Table = Table::new();
    table.add_row(header);

    let body: Row = ALLOWED_TYPES
        .into_iter()
        .map(|x| {
            let stmt = format!("SELECT COUNT(*) FROM HISTORY WHERE {} = 1", x);
            let mut stmt = conn.prepare(&stmt).unwrap();
            let res = stmt.query_row([], |row| row.get(0) as rusqlite::Result<i32>);
            Cell::new(&res.unwrap().to_string())
        })
        .collect();

    table.add_row(body);

    table.printstd();

    Ok(())
}
