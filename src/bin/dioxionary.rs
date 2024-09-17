//! StarDict in Rust!
//! Use offline or online dictionary to look up words and memorize words in the terminal!
use anyhow::Result;
use charcoal_dict::{app::config::Normal, word::QueryYoudict, Acquire, ExactQuery, PPrint};
use clap::CommandFactory;
use dioxionary::{
    cli::{Action, Cli, Parser},
    dict::is_enword,
    history, list_dicts, query_and_push_tty, query_fuzzy_interactive, repl, QueryStatus,
};
use shadow_rs::shadow;
use std::{env, path::PathBuf};

shadow!(build);

#[tokio::main]
async fn main() -> Result<()> {
    let cli: Cli = Cli::parse();

    if cli.version {
        println!("{}", build::VERSION); //print version const
        return Ok(());
    }

    if cli.online {
        if let Some(words) = &cli.word {
            for word in words {
                let exact_query = ExactQuery::new(word.clone()).unwrap();
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
                }
            }
        }
        return Ok(());
    }

    if let Some(shell) = cli.completions {
        let bin_name = env::args().next().expect("impossible");
        clap_complete::generate(shell, &mut Cli::command(), bin_name, &mut std::io::stdout());
        std::process::exit(0);
    }

    if let Some(action) = cli.action {
        match action {
            Action::Count => history::count_history(),
            Action::List(t) => history::list_history(t.type_, t.sort, t.table, t.column),
            Action::Lookup(w) => {
                let online = w.online;
                let local_first = w.local_first;
                let exact = w.exact_search;
                let word = w.word;
                let path = &w.local;
                let read_aloud = w.read_aloud;
                lookup(word, online, local_first, exact, path, read_aloud, true)
            }
            Action::Dicts => list_dicts(),
            Action::Review => {
                // dioxionary::sm2::review::main()
                dioxionary::fsrs::review::main()
            }
        }
    } else {
        lookup(
            cli.word,
            cli.online,
            cli.local_first,
            cli.exact_search,
            &cli.local,
            cli.read_aloud,
            !cli.non_interactive,
        )
    }
}

fn lookup(
    words: Option<Vec<String>>,
    online: bool,
    local_first: bool,
    exact: bool,
    path: &Option<String>,
    read_aloud: bool,
    interactive: bool,
) -> Result<()> {
    if let Some(word_list) = words {
        for word in word_list {
            let word = word.trim();
            let found = query_and_push_tty(word);
            if found != QueryStatus::NotFound && is_enword(word) {
                history::add_history(word.to_owned())?;
            }
            if found != QueryStatus::FoundLocally && interactive {
                let _ = query_fuzzy_interactive(word);
            }
        }
        Ok(())
    } else {
        repl(online, local_first, exact, path, read_aloud)
    }
}
