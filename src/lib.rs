#![feature(let_chains)]
#![feature(lazy_cell)]

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

use crate::dict::is_enword;
use crate::stardict::SearchAble;
use anyhow::{anyhow, Context, Result};
use charcoal_dict::Answer;
use charcoal_dict::{app::config::Normal, word::QueryYoudict, Acquire, ExactQuery, PPrint};
use dialoguer::{console::Term, theme::ColorfulTheme, Select};
use dirs::home_dir;
use itertools::Itertools;
use prettytable::{Attr, Cell, Row, Table};
use pulldown_cmark_mdcat_ratatui::markdown_widget::PathOrStr;
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::HistoryHinter;
use rustyline::history::DefaultHistory;
use rustyline::{Completer, Config, Helper, Hinter, Validator};
use stardict::{EntryWrapper, StarDict};
use std::borrow::Cow::{self, Borrowed, Owned};
use std::{fs::DirEntry, path::PathBuf};
use unicode::UnicodePicker;

/// Get the entries of the stardicts.
fn get_dicts_entries() -> Result<Vec<DirEntry>> {
    let mut dioxionary_dir = dirs::config_dir();
    let dioxionary_dir = dioxionary_dir
        .as_mut()
        .map(|dir| {
            dir.push("dioxionary");
            dir
        })
        .filter(|dir| dir.is_dir());

    let mut stardict_compatible_dir = dirs::home_dir();
    let stardict_compatible_dir = stardict_compatible_dir
        .as_mut()
        .map(|dir| {
            dir.push(".stardict");
            dir.push("dic");
            dir
        })
        .filter(|dir| dir.is_dir());

    let path = match (&dioxionary_dir, &stardict_compatible_dir) {
        (Some(dir), _) => dir,
        (None, Some(dir)) => dir,
        (None, None) => return Err(anyhow!("Couldn't find configuration directory")),
    };

    let mut dicts: Vec<_> = path
        .read_dir()
        .with_context(|| format!("Failed to open configuration directory {:?}", path))?
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
    let _ = dict::read_aloud(word);

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
    let mut rl = rustyline::Editor::<MyHelper, fsrs::Deck>::with_history(
        Config::default(),
        fsrs::Deck::default(),
    )?;
    rl.set_helper(Some(MyHelper(HistoryHinter::new())));
    let mut history: Vec<String> = Vec::new();
    loop {
        let readline = rl.readline("\x1b[34m>> \x1b[0m");
        match readline {
            Ok(word) => {
                let word = word.trim();
                if word == "similar"
                    || word == "similars"
                    || word == "similar words"
                    || word == "similar word"
                {
                    if let Some(last_word) = history.last() {
                        let similar_words = rl.history.fuzzy_lookup_in_history(last_word, 2);
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
                } else if !word.is_empty() {
                    let _ = rl.add_history_entry(word);
                    history.push(word.to_owned());
                    match query(&word) {
                        Ok((found, s)) => {
                            let s = s.iter().map(PathOrStr::get_str).join("\n\n");
                            println!("{s}");
                            if found != QueryStatus::NotFound && is_enword(word) {
                                history::add_history(word.to_owned())?;
                            }
                        }
                        Err(e) => {
                            eprintln!("{:?}", e);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // clear input when `ctrl+c`
                continue;
            }
            Err(ReadlineError::Eof) => break Ok(()),
            _ => break Err(anyhow!("Failed to read lines")),
        }
    }
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
