//! History API

use std::collections::VecDeque;
use std::collections::vec_deque;
use std::fs::File;
use std::iter::DoubleEndedIterator;
use std::ops::Index;
use std::path::Path;
#[cfg(unix)]
use libc;

use super::Result;
use super::config::{Config, HistoryDuplicates};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Forward,
    Reverse,
}

/// Current state of the history.
#[derive(Default)]
pub struct History {
    entries: VecDeque<String>,
    max_len: usize,
    ignore_space: bool,
    ignore_dups: bool,
}

impl History {
    pub fn new() -> History {
        Self::with_config(Config::default())
    }
    pub fn with_config(config: Config) -> History {
        History {
            entries: VecDeque::new(),
            max_len: config.max_history_size(),
            ignore_space: config.history_duplicates() == HistoryDuplicates::IgnoreConsecutive,
            ignore_dups: config.history_ignore_space(),
        }
    }

    /// Return the history entry at position `index`, starting from 0.
    pub fn get(&self, index: usize) -> Option<&String> {
        self.entries.get(index)
    }

    /// Return the last history entry (i.e. previous command)
    pub fn last(&self) -> Option<&String> {
        self.entries.back()
    }

    /// Add a new entry in the history.
    pub fn add<S: AsRef<str> + Into<String>>(&mut self, line: S) -> bool {
        if self.max_len == 0 {
            return false;
        }
        if line.as_ref().is_empty()
            || (self.ignore_space
                && line.as_ref()
                    .chars()
                    .next()
                    .map_or(true, |c| c.is_whitespace()))
        {
            return false;
        }
        if self.ignore_dups {
            if let Some(s) = self.entries.back() {
                if s == line.as_ref() {
                    return false;
                }
            }
        }
        if self.entries.len() == self.max_len {
            self.entries.pop_front();
        }
        self.entries.push_back(line.into());
        true
    }

    /// Returns the number of entries in the history.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Returns true if the history has no entry.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Set the maximum length for the history. This function can be called even
    /// if there is already some history, the function will make sure to retain
    /// just the latest `len` elements if the new history length value is smaller
    /// than the amount of items already inside the history.
    pub fn set_max_len(&mut self, len: usize) {
        self.max_len = len;
        if len == 0 {
            self.entries.clear();
            return;
        }
        loop {
            if self.entries.len() <= len {
                break;
            }
            self.entries.pop_front();
        }
    }

    /// Save the history in the specified file.
    pub fn save<P: AsRef<Path> + ?Sized>(&self, path: &P) -> Result<()> {
        use std::io::{BufWriter, Write};

        if self.is_empty() {
            return Ok(());
        }
        let old_umask = umask();
        let f = File::create(path);
        restore_umask(old_umask);
        let file = try!(f);
        fix_perm(&file);
        let mut wtr = BufWriter::new(file);
        for entry in &self.entries {
            try!(wtr.write_all(entry.as_bytes()));
            try!(wtr.write_all(b"\n"));
        }
        Ok(())
    }

    /// Load the history from the specified file.
    ///
    /// # Failure
    /// Will return `Err` if path does not already exist.
    pub fn load<P: AsRef<Path> + ?Sized>(&mut self, path: &P) -> Result<()> {
        use std::io::{BufRead, BufReader};

        let file = try!(File::open(&path));
        let rdr = BufReader::new(file);
        for line in rdr.lines() {
            self.add(try!(line).as_ref()); // TODO truncate to MAX_LINE
        }
        Ok(())
    }

    /// Clear history
    pub fn clear(&mut self) {
        self.entries.clear()
    }

    /// Search history (start position inclusive [0, len-1])
    /// Return the absolute index of the nearest history entry that matches `term`.
    /// Return None if no entry contains `term` between [start, len -1] for forward search
    /// or between [0, start] for reverse search.
    pub fn search(&self, term: &str, start: usize, dir: Direction) -> Option<usize> {
        let test = |entry: &String| entry.contains(term);
        self.search_match(term, start, dir, test)
    }

    pub fn starts_with(&self, term: &str, start: usize, dir: Direction) -> Option<usize> {
        let test = |entry: &String| entry.starts_with(term);
        self.search_match(term, start, dir, test)
    }

    fn search_match<F>(&self, term: &str, start: usize, dir: Direction, test: F) -> Option<usize>
    where
        F: Fn(&String) -> bool,
    {
        if term.is_empty() || start >= self.len() {
            return None;
        }
        match dir {
            Direction::Reverse => {
                let index = self.entries
                    .iter()
                    .rev()
                    .skip(self.entries.len() - 1 - start)
                    .position(test);
                index.and_then(|index| Some(start - index))
            }
            Direction::Forward => {
                let index = self.entries.iter().skip(start).position(test);
                index.and_then(|index| Some(index + start))
            }
        }
    }

    /// Return a forward iterator.
    pub fn iter(&self) -> Iter {
        Iter(self.entries.iter())
    }
}

impl Index<usize> for History {
    type Output = String;

    fn index(&self, index: usize) -> &String {
        &self.entries[index]
    }
}

impl<'a> IntoIterator for &'a History {
    type Item = &'a String;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

/// History iterator.
pub struct Iter<'a>(vec_deque::Iter<'a, String>);

impl<'a> Iterator for Iter<'a> {
    type Item = &'a String;

    fn next(&mut self) -> Option<&'a String> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<&'a String> {
        self.0.next_back()
    }
}

fn umask() -> libc::mode_t {
    unsafe { libc::umask(libc::S_IXUSR | libc::S_IRWXG | libc::S_IRWXO) }
}

fn restore_umask(old_umask: libc::mode_t) {
    unsafe {
        libc::umask(old_umask);
    }
}

fn fix_perm(file: &File) {
    use std::os::unix::io::AsRawFd;
    unsafe {
        libc::fchmod(file.as_raw_fd(), libc::S_IRUSR | libc::S_IWUSR);
    }
}
