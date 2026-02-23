#![feature(let_chains)]

//! StarDict in Rust!
//! Use offline or online dictionary to look up words and memorize words in the terminal!
pub mod cli;
pub mod dict;
pub mod fsrs;
pub mod history;
pub mod logseq;
pub mod review_helper;
pub mod spaced_repetition;
pub mod stardict;
pub mod theme;
pub mod unicode;

use crate::stardict::SearchAble;
use crate::stardict::{EntryWrapper, StarDict};
use crate::unicode::UnicodePicker;
use anyhow::{anyhow, Context, Result};
use charcoal_dict::Answer;
use charcoal_dict::{app::config::Normal, word::QueryYoudict, Acquire, ExactQuery, PPrint};
use dialoguer::{console::Term, theme::ColorfulTheme, Select};
use dirs::home_dir;
use prettytable::{Attr, Cell, Row, Table};
use pulldown_cmark_mdcat_ratatui::markdown_widget::PathOrStr;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::HistoryHinter;
use rustyline::{Completer, Config, Helper, Hinter, Validator};
use std::borrow::Cow::{self, Borrowed, Owned};
use std::fs::{create_dir_all, File};
use std::io::BufRead;
use std::io::BufReader;
use std::process::Command;
use std::{fs::DirEntry, path::PathBuf};

/// Get the entries of the stardicts.
fn get_dicts_entries() -> Result<Vec<DirEntry>> {
    let dioxionary_dir = dirs::config_dir()
        .map(|dir| dir.join("dioxionary"))
        .context("Couldn't find configuration directory")
        .unwrap();
    let _ = create_dir_all(&dioxionary_dir);

    let mut dicts: Vec<_> = dioxionary_dir
        .read_dir()
        .with_context(|| {
            format!(
                "Failed to open configuration directory {:?}",
                dioxionary_dir
            )
        })?
        .filter_map(|x| x.ok())
        .filter(|x| x.file_type().unwrap().is_dir())
        .collect();

    dicts.sort_by_key(|a| a.file_name());

    Ok(dicts)
}

fn get_dics() -> Vec<Box<dyn SearchAble>> {
    let mut dicts: Vec<Box<dyn SearchAble>> = vec![
        Box::new(logseq::Logseq {
            path: home_dir().unwrap().join("girl-logseq"),
        }),
        Box::new(logseq::Logseq {
            path: home_dir().unwrap().join("dictionary-logseq"),
        }),
        Box::new(logseq::Logseq {
            path: home_dir().unwrap().join("logseq-repo"),
        }),
    ];
    if let Ok(ds) = get_dicts_entries() {
        for d in ds {
            if let Ok(x) = StarDict::new(d.path()) {
                dicts.push(Box::new(x));
            }
        }
    }
    dicts.push(Box::new(UnicodePicker {
        path: home_dir()
            .unwrap()
            .join(".config/dioxionary/unipicker-symbols"),
    }));
    dicts.push(Box::new(UnicodePicker {
        path: home_dir()
            .unwrap()
            .join(".local/cheatsheet/personal/ubuntu"),
    }));
    dicts
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum QueryStatus {
    FoundLocally,
    FoundOnline,
    NotFound,
}

/// Look up a word with many flags.
///
/// # Params
/// - `online`: use online dictionary?
/// - `local_first`: Try offline dictionary first, then the online?
/// - `exact`: disable fuzzy searching?
/// - `word`: the word being looked up.
/// - `path`: the path of the stardict directory.
/// - `read_aloud`: play word pronunciation?
///
/// ## Word prefix
/// - `/terraria`: enable fuzzy searching.
/// - `|terraria`: disable fuzzy searching.
/// - `@terraria`: use online dictionary.
pub fn query(word: &str) -> Result<(QueryStatus, Vec<PathOrStr>)> {
    let mut v: Vec<PathOrStr> = get_dics()
        .into_iter()
        .filter_map(|d| d.exact_lookup(word))
        .collect();

    let mut found = if v.is_empty() {
        QueryStatus::NotFound
    } else {
        QueryStatus::FoundLocally
    };

    if found == QueryStatus::NotFound {
        let exact_query = ExactQuery::new(word.to_owned()).unwrap();
        if let Ok(single_query) = QueryYoudict::new().acquire(&exact_query)
            && let s = single_query.to_string()
            && !s.trim().is_empty()
        {
            v.push(PathOrStr::NormalStr(s));
            found = QueryStatus::FoundOnline;
        }
    }

    // ignore audio error
    if dict::read_aloud(word).is_err() {
        // espeak -s 50 "Your text here"
        let _ = Command::new("espeak")
            .arg("-s")
            .arg("50")
            .arg(word)
            .status(); // ignore error
    }

    Ok((found, v))
}

pub fn query_and_push_tty(word: &str) -> QueryStatus {
    let mut found = QueryStatus::NotFound;

    let dicts = get_dics();

    for d in &dicts {
        match d.push_tty(word) {
            Ok(_) => {
                found = QueryStatus::FoundLocally;
                println!("\n");
            }
            Err(_) => {}
        }
    }

    if found == QueryStatus::NotFound {
        let exact_query = ExactQuery::new(word.to_owned()).unwrap();
        if let Ok(single_query) = QueryYoudict::new().acquire(&exact_query) {
            single_query.pprint(
                &exact_query,
                &charcoal_dict::Config {
                    path: PathBuf::from("/"),
                    main_mode: charcoal_dict::app::config::MainMode::Normal,
                    speak: false,
                    normal: Normal {
                        with_pronunciation: false,
                        with_variants: true,
                        with_sentence: true,
                    },
                },
            );
            if !single_query.not_found() {
                found = QueryStatus::FoundOnline;
            }
        }
    }

    // ignore audio error
    let _ = dict::read_aloud(word);

    found
}

pub fn query_fuzzy_interactive(word: &str) -> Result<()> {
    let v = query_fuzzy(word);
    if !v.is_empty() {
        let mut last_selection = 0;
        loop {
            if let Some(selection) = Select::with_theme(&ColorfulTheme::default())
                .items(&v)
                .default(last_selection)
                .interact_on_opt(&Term::stderr())?
            {
                last_selection = selection;
                let EntryWrapper { entry, .. } = &v[selection];
                println!("{}\n{}\n", entry.word, entry.trans);
            }
        }
    } else {
        eprintln!("Nothing similar to mouth bit, sorry :(");
    }
    Ok(())
}

pub fn query_fuzzy(word: &str) -> Vec<EntryWrapper> {
    let dicts = get_dics().leak();

    let v = dicts
        .iter()
        .flat_map(|dict| {
            dict.fuzzy_lookup(word)
                .into_iter()
                .map(|entry| EntryWrapper {
                    dict_name: dict.dict_name(),
                    entry,
                })
        })
        .collect::<Vec<_>>();
    v
}

#[derive(Completer, Helper, Hinter, Validator)]
struct MyHelper(#[rustyline(Hinter)] HistoryHinter);

impl Highlighter for MyHelper {
    fn highlight_prompt<'b, 's: 'b, 'p: 'b>(
        &'s self,
        prompt: &'p str,
        default: bool,
    ) -> Cow<'b, str> {
        if default {
            Owned(format!("\x1b[1;32m{prompt}\x1b[m"))
        } else {
            Borrowed(prompt)
        }
    }

    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Owned(format!("\x1b[1m{hint}\x1b[m"))
    }
}

/// Look up a word with many flags interactively using [query].
pub fn repl(
    _online: bool,
    _local_first: bool,
    _exact: bool,
    _path: &Option<String>,
    _read_aloud: bool,
) -> Result<()> {
    let mut rl = rustyline::Editor::<MyHelper, fsrs::sqlite_history::SQLiteHistory>::with_history(
        Config::default(),
        fsrs::sqlite_history::SQLiteHistory::default(),
    )?;
    rl.set_helper(Some(MyHelper(HistoryHinter::new())));
    // let _ = rl.add_history_entry("similar");
    // let _ = rl.add_history_entry("similars");
    let mut history: Vec<String> = Vec::new();
    let mut no_result_words: Vec<String> = Vec::new();
    const THRESHOLD: usize = 2;
    loop {
        let readline = rl.readline("\x1b[34m>> \x1b[0m");
        match readline {
            Ok(word) => {
                let word = word.trim();
                match word {
                    "" => {}
                    "similar" | "similar words" | "similar word" => {
                        // search similar word in history
                        if let Some(last_word) = history.last() {
                            let similar_words =
                                rl.history().fuzzy_lookup_in_history(last_word, THRESHOLD);
                            if similar_words.is_empty() {
                                println!("not similar words found")
                            } else {
                                for x in similar_words {
                                    println!("{x}");
                                }
                            }
                        } else {
                            println!("no previous word")
                        }
                    }
                    "similars" | "leven" => {
                        // search similar word in /usr/share/dict/words
                        if let Some(last_word) = history.last() {
                            find_similar_words(last_word, THRESHOLD)?;
                        } else {
                            println!("no previous word")
                        }
                    }
                    _ => {
                        if let Some(distance) = word.strip_prefix("leven ") {
                            if let Some(last_word) = history.last() {
                                // `leven 1` : search similar word with levenshtein distance 1
                                let distance: usize = distance.trim().parse().unwrap();

                                find_similar_words(last_word, distance)?;
                            } else {
                                println!("no previous word")
                            }
                        } else {
                            let _ = rl.add_history_entry(word);
                            history.push(word.to_owned());
                            let found = query_and_push_tty(word);
                            if found == QueryStatus::NotFound {
                                no_result_words.push(word.to_owned());
                            }
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // clear input when `ctrl+c`
                continue;
            }
            Err(ReadlineError::Eof) => {
                for word in no_result_words {
                    // delete from sqlite
                    let _ = rl.history().delete(&word);
                }
                return Ok(());
            }
            _ => break Err(anyhow!("Failed to read lines")),
        }
    }
}

fn find_similar_words(last_word: &String, threshold: usize) -> Result<(), anyhow::Error> {
    println!("levenshtein({last_word}, X) ≤ {threshold}, X in /usr/share/dict/words");
    let file = File::open("/usr/share/dict/words")?;
    let sorted_lastword = sort_str(last_word);
    let reader = BufReader::new(file);
    for line in reader.lines() {
        let line = line?; // Handle potential I/O errors
        if strsim::levenshtein(&line, last_word) <= threshold || sort_str(&line) == sorted_lastword
        {
            println!("{line}");
        }
    }
    Ok(())
}

/// List stardicts in the dioxionary config path.
pub fn list_dicts() -> Result<()> {
    let mut table: Table = Table::new();
    table.add_row(Row::new(vec![
        Cell::new("Dictionary's name").with_style(Attr::Bold),
        Cell::new("Word count").with_style(Attr::Bold),
    ]));
    get_dicts_entries()?.into_iter().for_each(|x| {
        if let Ok(stardict) = StarDict::new(x.path()) {
            let row = Row::new(vec![
                Cell::new(stardict.dict_name()),
                Cell::new(stardict.wordcount().to_string().as_str()),
            ]);
            table.add_row(row);
        }
    });
    table.printstd();
    Ok(())
}

pub fn sort_str(s: &str) -> String {
    let mut chars: Vec<char> = s.chars().collect();

    // Sort the vector of characters
    chars.sort();

    // Collect the sorted characters back into a string
    chars.into_iter().collect()
}
