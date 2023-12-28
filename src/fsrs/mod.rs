use crate::spaced_repetition::SpacedRepetiton;
use anyhow::{Context, Result};
use chrono::DateTime;
use chrono::Duration;
use chrono::Local;
use dirs::data_dir;
use fsrs::MemoryState;
use fsrs::DEFAULT_WEIGHTS;
use fsrs::FSRS;
use serde::{Deserialize, Serialize};
use std::cell::LazyCell;
use std::fs;
use std::{collections::HashMap, path::PathBuf};

pub mod review;

#[derive(Serialize, Deserialize, Debug)]
struct MemoryStateWrapper {
    stability: f32,
    difficulty: f32,
    interval: u32,
    last_reviewed: DateTime<Local>,
}

impl Default for MemoryStateWrapper {
    fn default() -> Self {
        Self {
            stability: DEFAULT_WEIGHTS[0],
            difficulty: DEFAULT_WEIGHTS[4] + 2.0 * DEFAULT_WEIGHTS[5],
            interval: 1,
            last_reviewed: Local::now(),
        }
    }
}

impl MemoryStateWrapper {
    fn next_review_time(&self) -> DateTime<Local> {
        self.last_reviewed + Duration::days(self.interval.into())
    }

    fn to_memory_state(&self) -> MemoryState {
        MemoryState {
            stability: self.stability,
            difficulty: self.difficulty,
        }
    }
}

#[derive(Debug)]
pub struct Deck {
    fsrs: LazyCell<FSRS>,
    hm: HashMap<String, MemoryStateWrapper>,
}

impl Default for Deck {
    fn default() -> Self {
        Self {
            fsrs: LazyCell::new(|| FSRS::new(Some(&DEFAULT_WEIGHTS)).unwrap()),
            hm: Default::default(),
        }
    }
}

impl Deck {
    fn load_inner() -> Result<HashMap<String, MemoryStateWrapper>> {
        let path = get_json_location()?;
        let contents = std::fs::read_to_string(path)?;
        let hm = serde_json::from_str(&contents)?;
        Ok(hm)
    }
}

impl SpacedRepetiton for Deck {
    fn next_to_review(&self) -> Option<String> {
        for (k, v) in &self.hm {
            if v.next_review_time() <= Local::now() {
                return Some(k.to_owned());
            }
        }
        None
    }

    fn dump(&self) -> anyhow::Result<()> {
        let json_string = serde_json::to_string_pretty(&self.hm)?;
        let path = get_json_location()?;
        fs::write(path, json_string)?;
        Ok(())
    }

    fn load() -> Self {
        match Self::load_inner() {
            Ok(hm) => Self {
                fsrs: LazyCell::new(|| FSRS::new(Some(&DEFAULT_WEIGHTS)).unwrap()),
                hm,
            },
            Err(_) => Self {
                fsrs: LazyCell::new(|| FSRS::new(Some(&DEFAULT_WEIGHTS)).unwrap()),
                hm: Default::default(),
            },
        }
    }

    fn add_fresh_word(&mut self, w: String) {
        match self.hm.entry(w) {
            std::collections::hash_map::Entry::Occupied(_) => {}
            std::collections::hash_map::Entry::Vacant(v) => {
                v.insert(Default::default());
            }
        }
    }

    /// requires 1 <= q <= 4
    fn update(&mut self, question: String, q: u8) {
        let old_state = &self.hm[&question];
        let next_states = self
            .fsrs
            .next_states(
                Some(old_state.to_memory_state()),
                0.9,
                (Local::now() - old_state.last_reviewed)
                    .num_days()
                    .abs()
                    .try_into()
                    .unwrap(),
            )
            .unwrap();
        let new_memory_state = match q {
            1 => next_states.again,
            2 => next_states.hard,
            3 => next_states.good,
            4 => next_states.easy,
            _ => unreachable!(),
        };
        self.hm.insert(
            question,
            MemoryStateWrapper {
                stability: new_memory_state.memory.stability,
                difficulty: new_memory_state.memory.difficulty,
                interval: new_memory_state.interval,
                last_reviewed: Local::now(),
            },
        );
    }

    fn remove(&mut self, question: &str) {
        self.hm.remove(question);
    }
}

/// Check and generate cache directory path.
fn get_json_location() -> Result<PathBuf> {
    let mut path = data_dir().with_context(|| "Couldn't find cache directory")?;
    path.push("dioxionary");
    if !path.exists() {
        std::fs::create_dir(&path)
            .with_context(|| format!("Failed to create directory {:?}", path))?;
    }
    path.push("fsrs.json");
    Ok(path)
}
