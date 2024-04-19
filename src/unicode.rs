use dirs::home_dir;
use pulldown_cmark_mdcat_ratatui::markdown_widget::PathOrStr;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

/// https://github.com/jeremija/unipicker
use crate::stardict::SearchAble;

pub struct UnicodePicker;

impl SearchAble for UnicodePicker {
    fn exact_lookup(&self, word: &str) -> Option<PathOrStr> {
        let p = home_dir()?.join(".config/dioxionary/unipicker-symbols");
        let file = File::open(p).ok()?;
        let reader = BufReader::new(file);

        let mut s = String::new();

        'outer: for line in reader.lines().flatten() {
            let Some((line, _)) = line.split_once("  ") else {
                continue;
            };
            let v = line.split(' ');
            for x in v.skip(1) {
                if x.to_lowercase() == word {
                    s += line;
                    s += "\n";
                    continue 'outer;
                }
            }
        }

        if s.is_empty() {
            None
        } else {
            Some(PathOrStr::NormalStr(s))
        }
    }

    fn fuzzy_lookup(&self, target_word: &str) -> Vec<crate::stardict::Entry> {
        Vec::new()
    }

    fn dict_name(&self) -> &str {
        "unicodepicker"
    }
}
