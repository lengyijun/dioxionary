//! History query and addition using [sqlite](https://sqlite.org/index.html).
use crate::spaced_repetition::SpacedRepetiton;
use anyhow::{Context, Result};
use dirs::data_dir;
use prettytable::{Attr, Cell, Row, Table};
use rusqlite::Connection;
use std::fs::create_dir;

/// Check and generate cache directory path.
pub fn get_db() -> Result<Connection> {
    let mut path = data_dir().with_context(|| "Couldn't find data directory")?;
    path.push("dioxionary");
    if !path.exists() {
        create_dir(&path).with_context(|| format!("Failed to create directory {:?}", path))?;
    }
    path.push("dioxionary.db");
    let conn = Connection::open(path)?;
    let user_version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if user_version <= 0 {
        conn.execute_batch(
            "
BEGIN EXCLUSIVE;
CREATE TABLE fsrs (
word TEXT PRIMARY KEY,
difficulty REAL NOT NULL,
stability REAL NOT NULL,
interval INTEGER NOT NULL,
last_reviewed TEXT NOT NULL
);
CREATE VIRTUAL TABLE fsrs_fts USING fts4(content=fsrs, word);
CREATE TRIGGER history_bu BEFORE UPDATE ON fsrs BEGIN
    DELETE FROM fsrs_fts WHERE docid=old.rowid;
END;
CREATE TRIGGER history_bd BEFORE DELETE ON fsrs BEGIN
    DELETE FROM fsrs_fts WHERE docid=old.rowid;
END;
CREATE TRIGGER history_au AFTER UPDATE ON fsrs BEGIN
    INSERT INTO fsrs_fts (docid, word) VALUES (new.rowid, new.word);
END;
CREATE TRIGGER history_ai AFTER INSERT ON fsrs BEGIN
    INSERT INTO fsrs_fts (docid, word) VALUES(new.rowid, new.word);
END;
PRAGMA user_version = 1;
COMMIT;
",
        )?;
    }
    Ok(conn)
}

/// Add a looked up word to history.
pub fn add_history(word: String) -> Result<()> {
    let mut d = crate::fsrs::Deck::default();
    d.add_fresh_word(word)?;
    Ok(())
    // crate::sm2::Deck::add_history(word)
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
pub fn list_history(ttype: Option<String>, sort: bool, table: bool, column: usize) -> Result<()> {
    unreachable!()
}

/// Count the history.
pub fn count_history() -> Result<()> {
    unreachable!()
}
