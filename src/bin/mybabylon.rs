use anyhow::Context;
use dioxionary::stardict::{Idx, Ifo};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::env::args;
use std::fs::{self, File};
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use string_interner::StringInterner;

fn main() {
    let file_name = args().nth(1).expect("file_name expected");
    let file_path = PathBuf::from(file_name);
    let Ok(mut lines) = read_lines(&file_path) else {
        panic!("can't read file")
    };
    let mut interner = StringInterner::default();
    let mut btree_map: BTreeMap<(String, String), _> = BTreeMap::new();

    while let Some(line) = lines.next() {
        let Ok(line) = line else { continue };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut content = String::new();
        while let Some(Ok(line)) = lines.next() {
            if line.trim().is_empty() {
                break;
            }
            content.push('\n');
            content += &line;
        }
        if content.is_empty() {
            eprintln!("no content found for {line}");
            continue;
        }

        for key_word in line
            .split('|')
            .map(|word| word.trim())
            .filter(|word| !word.is_empty())
            .collect::<BTreeSet<_>>()
        {
            match btree_map.entry((key_word.to_lowercase(), key_word.to_owned())) {
                std::collections::btree_map::Entry::Vacant(v) => {
                    v.insert(interner.get_or_intern(content.clone()));
                }
                std::collections::btree_map::Entry::Occupied(mut o) => {
                    let acc = interner.resolve(*o.get()).unwrap();
                    let content = format!("{acc}\n{content}");
                    o.insert(interner.get_or_intern(content));
                }
            }
        }
    }

    let mut content: String = String::new();
    let values: HashSet<_> = btree_map.values().copied().collect();
    let values: HashMap<_, (u32, u32)> = values
        .into_iter()
        .map(|symbol| {
            let offset = content.len();
            content += interner.resolve(symbol).unwrap();
            let size = content.len() - offset;
            (
                symbol,
                (offset.try_into().unwrap(), size.try_into().unwrap()),
            )
        })
        .collect();

    let ifo = Ifo {
        version: dioxionary::stardict::Version::V242,
        bookname: file_path
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
        wordcount: btree_map.len(),
        synwordcount: 0,
        idxfilesize: 0,
        idxoffsetbits: 0,
        author: String::new(),
        email: String::new(),
        website: String::new(),
        description: String::new(),
        date: String::new(),
        sametypesequence: "m".to_string(),
        dicttype: String::new(),
    };
    fs::write(&file_path.with_extension("ifo"), ifo.to_string()).expect("can't write ifo");

    let items: Vec<(String, u32, u32)> = btree_map
        .into_iter()
        .map(|((_, k), symbol)| {
            let (offset, size) = values.get(&symbol).unwrap();
            (k, *offset, *size)
        })
        .collect();
    Idx::write_bytes(file_path.with_extension("idx"), items).expect("can't write idx");

    fs::write(file_path.with_extension("dict"), content)
        .with_context(|| format!("Failed to create dict file"))
        .unwrap();
}

// The output is wrapped in a Result to allow matching on errors
// Returns an Iterator to the Reader of the lines of the file.
fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where
    P: AsRef<Path>,
{
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
