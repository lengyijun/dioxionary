use anyhow::Result;
use dioxionary::fsrs::sqlite_history::SQLiteHistory;
use dioxionary::spaced_repetition::SpacedRepetiton;
use std::env::args;

fn main() -> Result<()> {
    let w = args().nth(1).unwrap_or("--help".to_owned());
    if w == "--help" {
        println!("used in goldendict-ng, program dic");
        println!("add first argument to sqlite");
        println!("https://github.com/lengyijun/dioxionary/tree/logseq");
        return Ok(());
    }
    let mut deck = SQLiteHistory::default();
    deck.add_fresh_word(w)?;
    Ok(())
}
