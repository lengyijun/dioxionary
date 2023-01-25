use crate::error::{Error, Result};
use eio::FromBytes;
use flate2::read::GzDecoder;
use std::cmp::min;
use std::fmt::Debug;
use std::fs::{read, File};
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;

#[allow(unused)]
pub struct StarDict {
    ifo: Ifo,
    idx: Idx,
    dict: Dict,
}

#[allow(unused)]
impl<'a> StarDict {
    pub fn new(path: PathBuf) -> Result<StarDict> {
        let dir = path.file_name().ok_or(Error::PathError)?;
        let dir = dir.to_str().unwrap();
        let ifo = Ifo::new(path.join(format!("{}.ifo", dir)))?;
        let idx = Idx::new(path.join(format!("{}.idx", dir)), ifo.version())?;
        let dict = Dict::new(path.join(format!("{}.dict.dz", dir)))?;
        Ok(StarDict { ifo, idx, dict })
    }

    fn fuzzy_search_for_best_match(&self, word: &str) -> Result<usize> {
        let pattern_chars: Vec<_> = word.to_lowercase().chars().collect();
        let idx = self
            .idx
            .items
            .iter()
            .enumerate()
            .filter(|(_, x)| !x.0.is_empty())
            .map(|(pos, x)| {
                let text_chars: Vec<_> = x.0.to_lowercase().chars().collect();
                let mut dist = vec![vec![0; pattern_chars.len() + 1]; text_chars.len() + 1];

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
                (dist[text_chars.len()][pattern_chars.len()], pos)
            })
            .filter(|x| x.0 <= pattern_chars.len() / 2)
            .min_by_key(|x| x.0)
            .ok_or(Error::WordNotFound(self.ifo.bookname.to_string()))?
            .1;
        Ok(idx)
    }

    pub fn lookup(&'a self, word: &str) -> Result<(&'a str, &'a str)> {
        if let Ok(pos) = self.idx.items.binary_search_by(|probe| {
            probe
                .0
                .to_lowercase()
                .cmp(&word.to_lowercase())
                .then(probe.0.as_str().cmp(&word))
        }) {
            let (ref word, offset, size) = self.idx.items[pos];
            Ok((word, self.dict.get(offset, size)))
        } else if let Ok(pos) = self.fuzzy_search_for_best_match(word) {
            let (ref word, offset, size) = self.idx.items[pos];
            Ok((word, self.dict.get(offset, size)))
        } else {
            Err(Error::WordNotFound(self.ifo.bookname.to_string()))
        }
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
/// description=	// You can use <br> for new line.
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

        for line in BufReader::new(File::open(path).map_err(|_| Error::CannotOpenIfoFile)?).lines()
        {
            let line = line?;
            if let Some(id) = line.find('=') {
                let key = &line[..id];
                let val = String::from(&line[id + 1..]);
                match key {
                    "version" => {
                        ifo.version = if val == "2.4.2" {
                            Version::V242
                        } else if val == "3.0.0" {
                            Version::V300
                        } else {
                            Version::Unknown
                        }
                    }
                    "bookname" => ifo.bookname = val,
                    "wordcount" => {
                        ifo.wordcount = val.parse().map_err(|_| Error::IfoFileParsingError)?
                    }
                    "synwordcount" => {
                        ifo.synwordcount = val.parse().map_err(|_| Error::IfoFileParsingError)?
                    }
                    "idxfilesize" => {
                        ifo.idxfilesize = val.parse().map_err(|_| Error::IfoFileParsingError)?
                    }
                    "idxoffsetbits" => {
                        ifo.idxoffsetbits = val.parse().map_err(|_| Error::IfoFileParsingError)?
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
impl<'a> Dict {
    fn new(path: PathBuf) -> Result<Dict> {
        let s = read(path).map_err(|x| Error::CannotOpenDictFile)?;
        let mut d = GzDecoder::new(s.as_slice());
        let mut contents = String::new();
        d.read_to_string(&mut contents)
            .map_err(|_| Error::DictFileError)?;
        Ok(Dict { contents })
    }

    fn get(&'a self, offset: usize, size: usize) -> &'a str {
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
    fn read_bytes<'a, const N: usize, T>(path: PathBuf) -> Result<Vec<(String, usize, usize)>>
    where
        T: FromBytes<N> + TryInto<usize>,
        <T as TryInto<usize>>::Error: Debug,
    {
        let f = File::open(path).map_err(|_| Error::CannotOpenIdxFile)?;
        let mut f = BufReader::new(f);

        let mut items: Vec<_> = Vec::new();
        let mut buf: Vec<u8> = Vec::new();

        while let Ok(n) = f.read_until(0, &mut buf) {
            if n == 0 {
                break;
            }

            buf.pop();
            let mut word = String::new();
            buf.iter().for_each(|x| word.push(*x as char));
            buf.clear();

            let mut b = [0; N];
            f.read(&mut b).map_err(|_| Error::IdxFileParsingError)?;
            let offset = T::from_be_bytes(b).try_into().unwrap();

            let mut b = [0; N];
            f.read(&mut b).map_err(|_| Error::IdxFileParsingError)?;
            let size = T::from_be_bytes(b).try_into().unwrap();

            items.push((word, offset, size))
        }
        Ok(items)
    }

    fn new(path: PathBuf, version: Version) -> Result<Idx> {
        match version {
            Version::V242 => Ok(Idx {
                items: Idx::read_bytes::<4, u32>(path)?,
            }),
            Version::V300 => Ok(Idx {
                items: Idx::read_bytes::<8, u64>(path)?,
            }),
            Version::Unknown => Err(Error::VersionError),
        }
    }
}
