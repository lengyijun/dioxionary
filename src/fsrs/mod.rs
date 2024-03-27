use crate::history;
use crate::spaced_repetition::SpacedRepetiton;
use anyhow::Result;
use chrono::DateTime;
use chrono::Duration;
use chrono::Local;
use fsrs::MemoryState;
use fsrs::DEFAULT_PARAMETERS;
use fsrs::FSRS;
use rusqlite::params;
use rusqlite::Connection;
use rusqlite::OptionalExtension;
use rustyline::error::ReadlineError;
use rustyline::history::History;
use rustyline::history::SearchDirection;
use rustyline::history::SearchResult;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::cell::LazyCell;
use std::str::FromStr;

pub mod review;

#[derive(Serialize, Deserialize, Debug)]
pub struct MemoryStateWrapper {
    pub stability: f32,
    pub difficulty: f32,
    pub interval: u32,
    pub last_reviewed: DateTime<Local>,
}

impl Default for MemoryStateWrapper {
    fn default() -> Self {
        Self {
            stability: DEFAULT_PARAMETERS[0],
            difficulty: DEFAULT_PARAMETERS[4] + 2.0 * DEFAULT_PARAMETERS[5],
            interval: 1,
            last_reviewed: Local::now(),
        }
    }
}

impl MemoryStateWrapper {
    pub fn next_review_time(&self) -> DateTime<Local> {
        self.last_reviewed + Duration::try_days(self.interval.into()).unwrap()
    }

    fn to_memory_state(&self) -> MemoryState {
        MemoryState {
            stability: self.stability,
            difficulty: self.difficulty,
        }
    }
}

#[derive(Debug)]
pub struct Deck {
    fsrs: LazyCell<FSRS>,
    conn: LazyCell<Connection>,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            fsrs: LazyCell::new(|| FSRS::new(Some(&DEFAULT_PARAMETERS)).unwrap()),
            conn: LazyCell::new(|| history::get_db().unwrap()),
        }
    }
}

impl SpacedRepetiton for Deck {
    fn next_to_review(&self) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT word, stability, difficulty, interval, last_reviewed FROM fsrs ORDER BY RANDOM()")?;
        let person_iter = stmt.query_map([], |row| {
            let time: String = row.get(4)?;
            let sm = MemoryStateWrapper {
                stability: row.get(1)?,
                difficulty: row.get(2)?,
                interval: row.get(3)?,
                last_reviewed: DateTime::<Local>::from_str(&time).unwrap(),
            };
            let word = row.get(0)?;
            Ok((word, sm))
        })?;
        for (word, sm) in person_iter.flatten() {
            if sm.next_review_time() <= Local::now() {
                return Ok(Some(word));
            }
        }
        Ok(None)
    }

    fn add_fresh_word(&mut self, word: String) -> Result<()> {
        insert_if_not_exists(&self.conn, &word, Default::default())?;
        Ok(())
    }

    /// requires 1 <= q <= 4
    fn update(&mut self, question: String, q: u8) -> Result<()> {
        let old_state = get_word(&self.conn, &question)?;
        let next_states = self.fsrs.next_states(
            Some(old_state.to_memory_state()),
            0.9,
            (Local::now() - old_state.last_reviewed)
                .num_days()
                .abs()
                .try_into()?,
        )?;
        let new_memory_state = match q {
            1 => next_states.again,
            2 => next_states.hard,
            3 => next_states.good,
            4 => next_states.easy,
            _ => unreachable!(),
        };
        let x = MemoryStateWrapper {
            stability: new_memory_state.memory.stability,
            difficulty: new_memory_state.memory.difficulty,
            interval: new_memory_state.interval,
            last_reviewed: Local::now(),
        };
        insert(&self.conn, &question, x)?;
        Ok(())
    }

    fn remove(&mut self, question: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM fsrs WHERE word = ?", [question])?;
        Ok(())
    }
}

fn insert_if_not_exists(conn: &Connection, word: &str, sm: MemoryStateWrapper) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO fsrs (word, stability, difficulty, interval, last_reviewed) VALUES (?1, ?2, ?3, ?4, ?5)",
        (word, sm.stability, sm.difficulty, sm.interval, sm.last_reviewed.to_string()),
    )?;
    Ok(())
}

fn insert(conn: &Connection, word: &str, sm: MemoryStateWrapper) -> Result<()> {
    conn.execute(
        "INSERT OR REPLACE INTO fsrs (word, stability, difficulty, interval, last_reviewed) VALUES (?1, ?2, ?3, ?4, ?5)",
        (word, sm.stability, sm.difficulty, sm.interval, sm.last_reviewed.to_string()),
    )?;
    Ok(())
}

fn get_word(conn: &Connection, word: &str) -> Result<MemoryStateWrapper> {
    let sm = conn.query_row(
        "SELECT stability, difficulty, interval, last_reviewed FROM fsrs WHERE word = ?",
        [word],
        |row| {
            let time: String = row.get(3)?;
            let sm = MemoryStateWrapper {
                stability: row.get(0)?,
                difficulty: row.get(1)?,
                interval: row.get(2)?,
                last_reviewed: DateTime::<Local>::from_str(&time).unwrap(),
            };
            Ok(sm)
        },
    )?;
    Ok(sm)
}

impl Deck {
    fn search_match(
        &self,
        term: &str,
        start: usize,
        dir: SearchDirection,
        start_with: bool,
    ) -> rustyline::Result<Option<SearchResult>> {
        if term.is_empty() || start >= self.len() {
            return Ok(None);
        }
        let start = start + 1; // first rowid is 1
        let query = match (dir, start_with) {
            (SearchDirection::Forward, true) => {
                "SELECT docid, word FROM fsrs_fts WHERE word MATCH '^' || ?1 || '*'  AND docid >= ?2 \
                 ORDER BY docid ASC LIMIT 1;"
            }
            (SearchDirection::Forward, false) => {
                "SELECT docid, word, offsets(fsrs_fts) FROM fsrs_fts WHERE word MATCH ?1 || '*'  AND docid \
                 >= ?2 ORDER BY docid ASC LIMIT 1;"
            }
            (SearchDirection::Reverse, true) => {
                "SELECT docid, word FROM fsrs_fts WHERE word MATCH '^' || ?1 || '*'  AND docid <= ?2 \
                 ORDER BY docid DESC LIMIT 1;"
            }
            (SearchDirection::Reverse, false) => {
                "SELECT docid, word, offsets(fsrs_fts) FROM fsrs_fts WHERE word MATCH ?1 || '*'  AND docid \
                 <= ?2 ORDER BY docid DESC LIMIT 1;"
            }
        };
        let mut stmt = self.conn.prepare_cached(query)?;
        let x = stmt
            .query_row((term, start), |r| {
                let rowid = r.get::<_, usize>(0)?;
                /*
                if rowid > self.row_id.get() {
                    self.row_id.set(rowid);
                }
                */
                Ok(SearchResult {
                    entry: Cow::Owned(r.get(1)?),
                    idx: rowid - 1, // rowid - 1
                    pos: if start_with {
                        term.len()
                    } else {
                        offset(r.get(2)?)
                    },
                })
            })
            .optional()?;
        Ok(x)
    }
}

impl History for Deck {
    fn get(
        &self,
        index: usize,
        dir: rustyline::history::SearchDirection,
    ) -> rustyline::Result<Option<rustyline::history::SearchResult>> {
        let rowid = index + 1; // first rowid is 1
        if self.is_empty() {
            return Ok(None);
        }
        // rowid may not be sequential
        let query = match dir {
            SearchDirection::Forward => {
                "SELECT rowid, word FROM fsrs WHERE rowid >= ?1 ORDER BY rowid ASC LIMIT 1;"
            }
            SearchDirection::Reverse => {
                "SELECT rowid, word FROM fsrs WHERE rowid <= ?1 ORDER BY rowid DESC LIMIT 1;"
            }
        };
        let mut stmt = self.conn.prepare_cached(query)?;
        stmt.query_row([rowid], |r| {
            let rowid = r.get::<_, usize>(0)?;
            /*
            if rowid > self.row_id.get() {
                self.row_id.set(rowid);
            }
             */
            Ok(SearchResult {
                entry: Cow::Owned(r.get(1)?),
                idx: rowid - 1,
                pos: 0,
            })
        })
        .optional()
        .map_err(ReadlineError::from)
    }

    fn add(&mut self, line: &str) -> rustyline::Result<bool> {
        // deal with in SpacedRepetiton
        Ok(true)
    }

    fn add_owned(&mut self, line: String) -> rustyline::Result<bool> {
        self.add(line.as_str())
    }

    fn len(&self) -> usize {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM fsrs").unwrap();
        stmt.query_row::<usize, _, _>(params![], |r| r.get(0))
            .unwrap()
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn set_max_len(&mut self, len: usize) -> rustyline::Result<()> {
        // never call
        Ok(())
    }

    fn ignore_dups(&mut self, yes: bool) -> rustyline::Result<()> {
        Ok(())
    }

    fn ignore_space(&mut self, yes: bool) {}

    fn save(&mut self, path: &std::path::Path) -> rustyline::Result<()> {
        todo!()
    }

    fn append(&mut self, path: &std::path::Path) -> rustyline::Result<()> {
        todo!()
    }

    fn load(&mut self, path: &std::path::Path) -> rustyline::Result<()> {
        todo!()
    }

    fn clear(&mut self) -> rustyline::Result<()> {
        // never call
        Ok(())
    }

    fn search(
        &self,
        term: &str,
        start: usize,
        dir: rustyline::history::SearchDirection,
    ) -> rustyline::Result<Option<rustyline::history::SearchResult>> {
        self.search_match(term, start, dir, false)
    }

    fn starts_with(
        &self,
        term: &str,
        start: usize,
        dir: rustyline::history::SearchDirection,
    ) -> rustyline::Result<Option<rustyline::history::SearchResult>> {
        self.search_match(term, start, dir, true)
    }
}

fn offset(s: String) -> usize {
    s.split(' ')
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
}
