use std::cmp;
use std::collections::BTreeSet;
use std::collections::HashMap;

pub mod engines;

use self::engines::Engine;
use super::history::History;

#[derive(Debug)]
pub struct Completion {
    rank: usize,
    pub replacement: String,
    pub description: String,
}

impl cmp::Ord for Completion {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.rank.cmp(&other.rank)
    }
}

impl cmp::PartialOrd for Completion {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl cmp::PartialEq for Completion {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank
    }
}

impl cmp::Eq for Completion {}

#[derive(Debug)]
pub struct Completions {
    sets: HashMap<String, BTreeSet<Completion>>,
    start: String,
    line: String,
}

impl Completions {
    pub fn len(&self) -> usize {
        let mut total = 0;
        for (_, ref v) in &self.sets {
            total += v.len();
        }
        total
    }

    pub fn pick_one<'a>(&'a self) -> Option<&'a Completion> {
        for (_, ref v) in &self.sets {
            for c in v.iter() {
                return Some(c);
            }
        }
        None
    }
}

pub struct Completer<'a> {
    engines: Vec<Box<Engine + 'a>>,
}

impl<'a> Completer<'a> {
    pub fn new(engines: Vec<Box<Engine + 'a>>) -> Completer<'a> {
        Completer { engines: engines }
    }
    pub fn completions(&mut self, start: &str, line: &[char]) -> Completions {
        let line_string = line.iter().cloned().collect::<String>();
        let mut sets = HashMap::new();
        for engine in &mut self.engines {
            use std::ops::DerefMut;
            let category = engine.category().to_owned();
            match engine.deref_mut().completions(start, &line_string) {
                Some(ref completions) => {
                    let mut set = BTreeSet::new();
                    for (index, compl) in completions.iter().enumerate() {
                        set.insert(Completion {
                            rank: index,
                            replacement: compl.0.to_owned().into_owned(),
                            description: compl.1.to_owned().into_owned(),
                        });
                    }
                    sets.insert(category.to_owned(), set);
                }
                None => {}
            }
        }
        Completions {
            sets: sets,
            start: start.into(),
            line: line_string,
        }
    }
}
