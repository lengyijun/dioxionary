use crate::sort_str;
use crate::spaced_repetition::SpacedRepetiton;
use anyhow::Result;
use chrono::Utc;
use rs_fsrs::Card;
use rs_fsrs::Rating;
use rusqlite::Connection;

pub mod review;
pub mod sqlite_history;

impl Default for sqlite_history::SQLiteHistory {
    fn default() -> Self {
        let builder = rustyline::config::Builder::new().auto_add_history(true);
        Self::open(builder.build()).unwrap()
    }
}

impl SpacedRepetiton for sqlite_history::SQLiteHistory {
    fn next_to_review(&self) -> Result<Option<String>> {
        let res = self.conn.query_row("SELECT word FROM fsrs WHERE timediff('now', substr(due, 2, length(due) - 2)) LIKE '+%' AND session_id < ?1 ORDER BY RANDOM() LIMIT 1;", (self.session_id,), |row|{row.get(0)})?;
        Ok(Some(res))
    }

    fn add_fresh_word(&mut self, word: String) -> Result<()> {
        self.create_session()?;
        self.add_entry_ignore(&word, Default::default())?;
        Ok(())
    }

    /// requires 1 <= q <= 4
    fn update(&mut self, question: String, rating: Rating) -> Result<()> {
        let old_state = get_word(&self.conn, &question)?;
        let scheduling_info = self.fsrs.next(old_state, Utc::now(), rating);
        update(&self.conn, &question, scheduling_info.card)?;
        Ok(())
    }

    fn remove(&mut self, question: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM fsrs WHERE word = ?", [question])?;
        Ok(())
    }
}

fn update(conn: &Connection, word: &str, card: Card) -> Result<()> {
    conn.execute("INSERT OR REPLACE INTO fsrs (session_id, word, due, stability, difficulty, elapsed_days, scheduled_days, reps, lapses, state, last_review) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) RETURNING rowid;",
    (
         0, // TODO: session_id
         word,
         serde_json::to_string(&card.due)?,
         card.stability,
         card.difficulty,
         card.elapsed_days,
         card.scheduled_days,
         card.reps,
         card.lapses,
         serde_json::to_string(&card.state)?,
         serde_json::to_string(&card.last_review)?,
    )
             )?;

    Ok(())
}

fn get_word(conn: &Connection, word: &str) -> Result<Card> {
    let sm = conn.query_row("SELECT due, stability, difficulty, elapsed_days, scheduled_days, reps, lapses, state, last_review
    FROM fsrs WHERE word = ?", [word], |sqlite_row| {
        let x0: String = sqlite_row.get(0)?;
        let x7: String = sqlite_row.get(7)?;
        let x8: String = sqlite_row.get(8)?;
        let card: Card = Card {
            due: serde_json::from_str(&x0).unwrap(),
            stability: sqlite_row.get(1)?,
            difficulty: sqlite_row.get(2)?,
            elapsed_days: sqlite_row.get(3)?,
            scheduled_days: sqlite_row.get(4)?,
            reps: sqlite_row.get(5)?,
            lapses: sqlite_row.get(6)?,
            state: serde_json::from_str(&x7).unwrap(),
            last_review: serde_json::from_str(&x8).unwrap(),
        };
        Ok(card)
    })?;
    Ok(sm)
}

impl sqlite_history::SQLiteHistory {
    pub fn fuzzy_lookup_in_history(&self, target_word: &str, threhold: usize) -> Vec<String> {
        let sorted_targetword = sort_str(target_word);
        let mut stmt = self.conn.prepare("SELECT word FROM fsrs").unwrap();
        stmt.query_map([], |row| {
            let word: String = row.get(0).unwrap();
            if strsim::levenshtein(&word, target_word) <= threhold
                || sort_str(&word) == sorted_targetword
            {
                Ok(word)
            } else {
                Err(rusqlite::Error::ExecuteReturnedResults)
            }
        })
        .unwrap()
        .flatten()
        .collect()
    }
}
