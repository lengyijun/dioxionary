use anyhow::Result;
use rs_fsrs::Rating;

pub trait SpacedRepetiton: Sized + Default {
    /// find next reviewable word
    fn next_to_review(&self) -> Result<Option<String>>;

    fn add_fresh_word(&mut self, w: String) -> Result<()>;

    fn update(&mut self, question: String, rating: Rating) -> Result<()>;

    fn remove(&mut self, question: &str) -> Result<()>;
}
