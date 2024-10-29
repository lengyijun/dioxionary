/// replaced with `cat $1 | grep -w -i $2`
///
/// used for goldendict
/// https://xiaoyifang.github.io/goldendict-ng/howto/how%20to%20add%20a%20program%20as%20dictionary/
/// gd_txt /home/lyj/.config/dioxionary/unipicker-symbols %GDWORD%
/// gd_txt /home/lyj/.local/cheatsheet/personal/ubuntu %GDWORD%
use dioxionary::stardict::SearchAble;
use dioxionary::unicode::UnicodePicker;
use std::env::args;
use std::path::PathBuf;

fn main() -> anyhow::Result<()> {
    let path = args().nth(1).expect("dir expected");
    let mut word = args().nth(2).expect("word expected");
    if word.starts_with("/") {
        word = word[1..].to_owned();
    }

    let unicode_picker = UnicodePicker {
        path: PathBuf::from(path),
    };
    unicode_picker.push_tty(&word)
}
