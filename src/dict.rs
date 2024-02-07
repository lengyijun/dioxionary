//! Look up words from the Internet.
use anyhow::{anyhow, Context, Result};
use itertools::{
    EitherOrBoth::{Both, Left, Right},
    Itertools,
};
use rodio::{Decoder, OutputStream, Sink};
use scraper::{Html, Selector};
use std::fmt;
use std::io::Cursor;

/// Is an English word?
pub fn is_enword(word: &str) -> bool {
    word.chars()
        .all(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace())
}

/// Play word pronunciation.
pub fn read_aloud(word: &str) -> Result<()> {
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let url = format!("https://dict.youdao.com/dictvoice?audio={}&type=1", word);
    let response = reqwest::blocking::get(url)?;
    let inner = response.bytes()?;
    if let Ok(source) = Decoder::new(Cursor::new(inner)) {
        if let Ok(sink) = Sink::try_new(&stream_handle) {
            sink.append(source);
            sink.sleep_until_end();
        }
    }
    Ok(())
}
