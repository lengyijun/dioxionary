/// deal with unipicker-symbols  
use std::io::{self, BufRead};

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        let line = line?;
        let Some((line, _)) = line.split_once("  ") else {
            continue;
        };
        println!("{line}");
    }
    Ok(())
}
