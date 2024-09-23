//! Look up words from the Internet.
use anyhow::Result;
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;

/// Is an English word?
pub fn is_enword(word: &str) -> bool {
    word.chars()
        .all(|c| c.is_ascii_alphanumeric() || c.is_ascii_whitespace())
}

/// Play word pronunciation.
pub fn read_aloud(word: &str) -> Result<()> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let url = format!("https://dict.youdao.com/dictvoice?audio={}&type=1", word);
    let response = reqwest::blocking::get(url)?;
    let inner = response.bytes()?;
    let source = Decoder::new(Cursor::new(inner))?;
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
