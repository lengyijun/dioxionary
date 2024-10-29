//! used for goldendict
//! https://xiaoyifang.github.io/goldendict-ng/howto/how%20to%20add%20a%20program%20as%20dictionary/
//! clove /home/lyj/logseq-repo %GDWORD%
use anyhow::anyhow;
use dioxionary::logseq::Logseq;
use std::env::args;
use std::path::PathBuf;

fn main() {
    let _ = try_main();
}

fn try_main() -> anyhow::Result<()> {
    let path = args().nth(1).expect("dir expected");
    let mut word = args().nth(2).expect("word expected");
    if word.starts_with("/") {
        word = word[1..].to_owned();
    }

    let logseq = Logseq {
        path: PathBuf::from(path),
    };

    let Some(de) = logseq.find_path(&word) else {
        return Err(anyhow!("not found"));
    };
    let Some(path) = de.path().to_str() else {
        return Err(anyhow!("to_str fail"));
    };

    std::env::set_var("MDPATH", path);

    let html = markdown::to_html_with_options(
        &std::fs::read_to_string(PathBuf::from(path))?,
        &markdown::Options {
            compile: markdown::CompileOptions {
                allow_dangerous_html: true,
                allow_dangerous_protocol: true,
                ..markdown::CompileOptions::default()
            },
            ..markdown::Options::default()
        },
    );

    match html {
        Ok(html) => {
            println!("{html}",);
            Ok(())
        }
        Err(e) => {
            eprintln!("{:?}", e);
            Err(anyhow!("to_html fail"))
        }
    }
}
