use anyhow::Result;
use dioxionary::query_fuzzy;
use std::env;

fn main() -> Result<()> {
    let word = env::args().nth(1).unwrap();
    let v = query_fuzzy(&word);
    for x in v {
        println!("{x}");
    }
    Ok(())
}
