use anyhow::Result;

pub trait SpacedRepetiton: Sized {
    /// find next reviewable word
    fn next_to_review(&self) -> Option<String>;

    /// save to disk
    fn dump(&self) -> Result<()>;

    /// load from disk or default
    fn load() -> Self;

    /// add entry when lookup a new word
    fn add_fresh_word(&mut self, w: String);

    fn add_history(w: String) -> Result<()> {
        let mut x = Self::load();
        x.add_fresh_word(w);
        x.dump()
    }

    fn update(&mut self, question: String, q: u8);

    fn update_and_dump(&mut self, question: String, q: u8) -> Result<()> {
        self.update(question, q);
        self.dump()
    }

    fn remove(&mut self, question: &str);
}
