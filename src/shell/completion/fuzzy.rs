use super::*;
use std::cmp;
use std::collections::BTreeSet;
extern crate strsim;
use self::strsim::*;

#[cfg(feature = "parallel-search")]
extern crate rayon;
#[cfg(feature = "parallel-search")]
use self::rayon::prelude::*;

#[derive(Debug)]
struct RefCompletion<'a> {
    str_distance: f64,
    prefix_length: usize,
    suffix_length: usize,
    original_rank: usize,
    pub replacement: &'a str,
    pub description: &'a str,
}

impl<'a> cmp::Ord for RefCompletion<'a> {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.str_distance.partial_cmp(&other.str_distance).unwrap_or(cmp::Ordering::Equal).reverse()
            .then(self.prefix_length.cmp(&other.prefix_length).reverse())
            .then(self.suffix_length.cmp(&other.suffix_length).reverse())
            .then(self.original_rank.cmp(&other.original_rank))
    }
}

impl<'a> cmp::PartialOrd for RefCompletion<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<'a> cmp::PartialEq for RefCompletion<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == cmp::Ordering::Equal
    }
}

impl<'a> cmp::Eq for RefCompletion<'a> {}

fn longest_common_prefix(first: &str, second: &str) -> usize {
    first.chars().zip(second.chars()).take_while(|&(f,s)| f == s).count()
}

fn longest_common_suffix(first: &str, second: &str) -> usize {
    first.chars().rev().zip(second.chars().rev()).take_while(|&(f,s)| f == s).count()
}

#[cfg(feature = "parallel-search")]
const PARALLEL_THRESHOLD: usize = 1000;

pub fn fuzzy_search(original_set: &BTreeSet<Completion>, search_term: &str) -> BTreeSet<Completion> {
    let mut result = BTreeSet::new();

    fn compute_distances<'a>(c: &'a Completion, search_term: &str) -> Option<RefCompletion<'a>> {
        let distance = jaro(search_term, &c.replacement);
        let max_distance = search_term.len() + c.replacement.len();
        if distance > 0 as f64 {
            Some(RefCompletion {
                str_distance: distance,
                prefix_length: longest_common_prefix(search_term, &c.replacement),
                suffix_length: longest_common_suffix(search_term, &c.replacement),
                original_rank: c.rank,
                replacement: &c.replacement,
                description: &c.description,
            })
        } else {
            None
        }
    };

    #[cfg(feature = "parallel-search")]
    fn search<'a>(original_set: &'a BTreeSet<Completion>, search_term: &str) -> Vec<RefCompletion<'a>> {
        if original_set.len() > PARALLEL_THRESHOLD {
            original_set.par_iter().filter_map(|c| compute_distances(c, search_term)).collect()
        } else {
            original_set.iter().filter_map(|c| compute_distances(c, search_term)).collect()
        }
    }

    #[cfg(not(feature = "parallel-search"))]
    fn search<'a>(original_set: &'a BTreeSet<Completion>, search_term: &str) -> Vec<RefCompletion<'a>> {
        original_set.iter().filter_map(|c| compute_distances(c, search_term)).collect()
    }

    let mut refs: Vec<RefCompletion> = search(original_set, search_term);

    refs.sort();
    refs.into_iter().enumerate().for_each(|(index, o)| {
        result.insert(Completion {
            rank: index,
            replacement: o.replacement.into(),
            description: o.description.into(),
        });
    });
    panic!("searched");
    result
}
