//! Look up words form the offline stardicts.
use anyhow::{anyhow, Context, Result};
use eio::FromBytes;
use flate2::read::GzDecoder;
use std::borrow::Cow;
use std::cmp::min;
use std::fmt::Debug;
use std::fs::{read, File};
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;

pub trait SearchAble {
    fn exact_lookup(&self, word: &str) -> Option<Entry>;
    fn fuzzy_lookup(&self, target_word: &str) -> Vec<Entry>;
    fn dict_name(&self) -> &str;
}

/// The stardict to be looked up.
#[allow(unused)]
pub struct StarDict {
    ifo: Ifo,
    idx: Idx,
    dict: Dict,
}

/// A word entry of the stardict.
pub struct Entry<'a> {
    pub word: String,
    pub trans: Cow<'a, str>,
}

// only used in fuzzy search selection
pub struct EntryWrapper<'a, 'b> {
    pub dict_name: &'b str,
    pub entry: Entry<'a>,
}

impl std::fmt::Display for EntryWrapper<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "{} {}", self.entry.word, self.dict_name)
    }
}

impl StarDict {
    pub fn new(path: PathBuf) -> Result<StarDict> {
        let mut ifo: Option<_> = None;
        let mut idx: Option<_> = None;
        let mut dict: Option<_> = None;

        for path in path
            .read_dir()
            .with_context(|| format!("Failed to open directory {:?}", path))?
            .flatten()
        {
            let path = path.path();
            if let Some(extension) = path.extension() {
                match extension.to_str().unwrap() {
                    "ifo" => ifo = Some(path),
                    "idx" => idx = Some(path),
                    "dz" => dict = Some(path),
                    _ => (),
                }
            }
        }

        if ifo.is_none() || idx.is_none() || dict.is_none() {
            return Err(anyhow!("Stardict file is incomplete in {:?}", path));
        }

        let ifo = Ifo::new(ifo.unwrap())?;
        let idx = Idx::new(idx.unwrap(), ifo.version())?;
        let dict = Dict::new(dict.unwrap())?;

        /*
        idx.items
            .retain(|(_word, offset, size)| offset + size <= dict.contents.len());
         */

        Ok(StarDict { ifo, idx, dict })
    }

    /// Get the number of the words in the stardict.
    pub fn wordcount(&self) -> usize {
        self.ifo.wordcount
    }
}

impl SearchAble for StarDict {
    fn exact_lookup(&self, word: &str) -> Option<Entry> {
        let word = word.to_lowercase();
        if let Ok(pos) = self
            .idx
            .items
            .binary_search_by(|probe| probe.0.to_lowercase().cmp(&word))
        {
            let (word, offset, size) = &self.idx.items[pos];
            let trans = self.dict.get(*offset, *size);
            Some(Entry {
                word: word.to_string(),
                trans: std::borrow::Cow::Borrowed(trans),
            })
        } else {
            None
        }
    }

    fn fuzzy_lookup(&self, target_word: &str) -> Vec<Entry> {
        let target_word = target_word.to_lowercase();
        // bury vs buried
        let mut min_dist = 3;
        let mut res: Vec<&(String, usize, usize)> = Vec::new();

        for x in self.idx.items.iter() {
            let (word, _offset, _size) = x;
            let dist = min_edit_distance(&target_word, &word.to_lowercase());
            match dist.cmp(&min_dist) {
                std::cmp::Ordering::Less => {
                    min_dist = dist;
                    res.clear();
                    res.push(x);
                }
                std::cmp::Ordering::Equal => {
                    res.push(x);
                }
                std::cmp::Ordering::Greater => {}
            }
        }

        res.into_iter()
            .map(|(word, offset, size)| Entry {
                word: word.to_string(),
                trans: std::borrow::Cow::Borrowed(self.dict.get(*offset, *size)),
            })
            .collect()
    }

    fn dict_name(&self) -> &str {
        &self.ifo.bookname
    }
}

/// bookname=      // required
/// wordcount=     // required
/// synwordcount=  // required if ".syn" file exists.
/// idxfilesize=   // required
/// idxoffsetbits= // New in 3.0.0
/// author=
/// email=
/// website=
/// description=   // You can use <br> for new line.
/// date=
/// sametypesequence= // very important.
/// dicttype=

#[allow(unused)]
#[derive(Debug)]
struct Ifo {
    version: Version,
    bookname: String,
    wordcount: usize,
    synwordcount: usize,
    idxfilesize: usize,
    idxoffsetbits: usize,
    author: String,
    email: String,
    website: String,
    description: String,
    date: String,
    sametypesequence: String,
    dicttype: String,
}

#[derive(Debug, Clone, Copy)]
enum Version {
    V242,
    V300,
    Unknown,
}

impl Version {
    const V242_STR: &'static str = "2.4.2";
    const V300_STR: &'static str = "3.0.0";

    fn to_string(&self) -> &'static str {
        match self {
            Version::V242 => Self::V242_STR,
            Version::V300 => Self::V300_STR,
            Version::Unknown => panic!("Unknown.to_string()"),
        }
    }
}

#[allow(unused)]
impl Ifo {
    fn new(path: PathBuf) -> Result<Ifo> {
        let mut ifo = Ifo {
            version: Version::Unknown,
            bookname: String::new(),
            wordcount: 0,
            synwordcount: 0,
            idxfilesize: 0,
            idxoffsetbits: 0,
            author: String::new(),
            email: String::new(),
            website: String::new(),
            description: String::new(),
            date: String::new(),
            sametypesequence: String::new(),
            dicttype: String::new(),
        };

        for line in BufReader::new(
            File::open(&path).with_context(|| format!("Failed to open ifo file {:?}", path))?,
        )
        .lines()
        {
            let line = line?;
            if let Some(id) = line.find('=') {
                let key = &line[..id];
                let val = String::from(&line[id + 1..]);
                match key {
                    "version" => {
                        ifo.version = if val == Version::V242_STR {
                            Version::V242
                        } else if val == Version::V300_STR {
                            Version::V300
                        } else {
                            Version::Unknown
                        }
                    }
                    "bookname" => ifo.bookname = val,
                    "wordcount" => {
                        ifo.wordcount = val
                            .parse()
                            .with_context(|| format!("Failed to parse info file {:?}", path))?
                    }
                    "synwordcount" => {
                        ifo.synwordcount = val
                            .parse()
                            .with_context(|| format!("Failed to parse info file {:?}", path))?
                    }
                    "idxfilesize" => {
                        ifo.idxfilesize = val
                            .parse()
                            .with_context(|| format!("Failed to parse info file {:?}", path))?
                    }
                    "idxoffsetbits" => {
                        ifo.idxoffsetbits = val
                            .parse()
                            .with_context(|| format!("Failed to parse info file {:?}", path))?
                    }
                    "author" => ifo.author = val,
                    "email" => ifo.email = val,
                    "website" => ifo.website = val,
                    "description" => ifo.description = val,
                    "date" => ifo.date = val,
                    "sametypesequence" => ifo.sametypesequence = val,
                    "dicttype" => ifo.dicttype = val,
                    _ => (),
                };
            }
        }
        Ok(ifo)
    }

    fn version(&self) -> Version {
        self.version
    }
}

#[allow(unused)]
struct Dict {
    contents: String,
}

#[allow(unused)]
impl Dict {
    fn new(path: PathBuf) -> Result<Dict> {
        let s =
            read(&path).with_context(|| format!("Failed to open stardict directory {:?}", path))?;
        let mut d = GzDecoder::new(s.as_slice());
        let mut contents = String::new();
        d.read_to_string(&mut contents).with_context(|| {
            format!("Failed to open stardict directory {:?} as dz format", path)
        })?;
        Ok(Dict { contents })
    }

    fn get(&self, offset: usize, size: usize) -> &str {
        &self.contents[offset..offset + size]
    }
}

#[allow(unused)]
#[derive(Debug)]
struct Idx {
    items: Vec<(String, usize, usize)>,
}

#[allow(unused)]
impl Idx {
    fn read_bytes<const N: usize, T>(path: PathBuf) -> Result<Self>
    where
        T: FromBytes<N> + TryInto<usize>,
        <T as TryInto<usize>>::Error: Debug,
    {
        let f = File::open(&path).with_context(|| format!("Failed to open idx file {:?}", path))?;
        let mut f = BufReader::new(f);

        let mut items: Vec<_> = Vec::new();

        let mut buf: Vec<u8> = Vec::new();
        let mut b = [0; N];
        loop {
            buf.clear();

            let read_bytes = f
                .read_until(0, &mut buf)
                .with_context(|| format!("Failed to parse idx file {:?}", path))?;

            if read_bytes == 0 {
                break;
            }

            if buf.last() == Some(&b'\0') {
                buf.pop();
            }

            let word: String = String::from_utf8_lossy(&buf)
                .chars()
                .filter(|&c| c != '\u{fffd}')
                .collect();

            f.read(&mut b)
                .with_context(|| format!("Failed to parse idx file {:?}", path))?;
            let offset = T::from_be_bytes(b).try_into().unwrap();

            f.read(&mut b)
                .with_context(|| format!("Failed to parse idx file {:?}", path))?;
            let size = T::from_be_bytes(b).try_into().unwrap();

            if !word.is_empty() {
                items.push((word, offset, size))
            }
        }
        Ok(Self { items })
    }

    fn new(path: PathBuf, version: Version) -> Result<Idx> {
        match version {
            Version::V242 => Ok(Idx::read_bytes::<4, u32>(path)?),
            Version::V300 => Ok(Idx::read_bytes::<8, u64>(path)?),
            Version::Unknown => Err(anyhow!("Wrong stardict version in idx file {:?}", path)),
        }
    }
}

#[cfg(test)]
mod test {
    use itertools::izip;

    use super::StarDict;

    #[test]
    fn load_stardict() {
        let stardict = StarDict::new("./stardict-heritage/cdict-gb".into()).unwrap();
        assert_eq!(stardict.dict_name(), "CDICT5英汉辞典");
        assert_eq!(stardict.wordcount(), 57510);
    }

    #[test]
    fn lookup_offline() {
        let stardict = StarDict::new("./stardict-heritage/cdict-gb".into()).unwrap();
        stardict.exact_lookup("rust").unwrap();
    }

    #[test]
    fn lookup_offline_fuzzy() {
        let stardict = StarDict::new("./stardict-heritage/cdict-gb".into()).unwrap();
        let misspell = ["rst", "cago", "crade"];
        let correct = ["rust", "cargo", "crate"];
        for (mis, cor) in izip!(misspell, correct) {
            let fuzzy = stardict.fuzzy_lookup(mis).unwrap();
            fuzzy.iter().find(|w| w.word == cor).unwrap();
        }
    }
}

fn min_edit_distance(pattern: &str, text: &str) -> usize {
    let pattern_chars: Vec<_> = pattern.chars().collect();
    let text_chars: Vec<_> = text.chars().collect();
    let mut dist = vec![vec![0; pattern_chars.len() + 1]; text_chars.len() + 1];

    #[allow(clippy::needless_range_loop)]
    for i in 0..=text_chars.len() {
        dist[i][0] = i;
    }

    for j in 0..=pattern_chars.len() {
        dist[0][j] = j;
    }

    for i in 1..=text_chars.len() {
        for j in 1..=pattern_chars.len() {
            dist[i][j] = if text_chars[i - 1] == pattern_chars[j - 1] {
                dist[i - 1][j - 1]
            } else {
                min(min(dist[i][j - 1], dist[i - 1][j]), dist[i - 1][j - 1]) + 1
            }
        }
    }
    dist[text_chars.len()][pattern_chars.len()]
}
