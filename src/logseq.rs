use crate::stardict::Entry;
use crate::stardict::SearchAble;
use dirs::home_dir;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use walkdir::WalkDir;

pub struct Logseq {}

impl SearchAble for Logseq {
    fn exact_lookup<'a>(&'a self, word: &str) -> Option<Entry<'a>> {
        self.find(word)
    }

    fn fuzzy_lookup<'a>(&'a self, target_word: &str) -> Vec<Entry<'a>> {
        match self.find(target_word) {
            Some(x) => vec![x],
            None => vec![],
        }
    }

    fn dict_name(&self) -> &str {
        "logseq"
    }
}

impl Logseq {
    fn find<'a>(&'a self, word: &str) -> Option<Entry<'a>> {
        let word = word.to_lowercase();
        let root = home_dir().unwrap().join("girl-logseq").join("pages");
        'outer: for entry in WalkDir::new(root) {
            let Ok(entry) = entry else { continue };
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            let Some(file_name) = entry.file_name().to_str() else {
                continue;
            };
            if file_name.to_lowercase() == format!("{word}.md") {
                return Some(Entry {
                    word: word.to_string(),
                    trans: std::borrow::Cow::Owned(read_file_to_string(path)),
                });
            }
            if let Some((_, file_name)) = file_name.rsplit_once("%2F")
                && file_name.to_lowercase() == format!("{word}.md")
            {
                return Some(Entry {
                    word: word.to_string(),
                    trans: std::borrow::Cow::Owned(read_file_to_string(path)),
                });
            }

            let Ok(file) = File::open(path) else { continue };
            let reader = BufReader::new(file);
            for line in reader.lines() {
                let Ok(line) = line else { continue 'outer };
                static ALIAS: &str = "alias:: ";
                if line.starts_with(ALIAS) {
                    let Some(line) = line.get(ALIAS.len()..) else {
                        unreachable!()
                    };
                    if line.split(',').any(|x| x.trim().to_lowercase() == word) {
                        return Some(Entry {
                            word: word.to_string(),
                            trans: std::borrow::Cow::Owned(read_file_to_string(path)),
                        });
                    }
                } else if line.starts_with("- ") {
                    break;
                } else {
                    // other property of page
                }
            }
        }
        None
    }
}

fn read_file_to_string(path: &Path) -> String {
    let contents = std::fs::read_to_string(path).unwrap();
    format!(
        "{}\n{contents}",
        path.file_name().unwrap().to_str().unwrap(),
    )
}
