//! Line buffer with current cursor position
use std::iter;
use std::ops::{Deref, Range};
use super::unicode_segmentation::UnicodeSegmentation;
use super::keymap::{At, CharSearch, Movement, RepeatCount, Word};

/// Maximum buffer size for the line read
pub static MAX_LINE: usize = 4096;

pub enum WordAction {
    CAPITALIZE,
    LOWERCASE,
    UPPERCASE,
}

#[derive(Debug)]
pub struct LineBuffer {
    buf: String, // Edited line buffer
    pos: usize,  // Current cursor position (byte position)
}

impl LineBuffer {
    /// Create a new line buffer with the given maximum `capacity`.
    pub fn with_capacity(capacity: usize) -> LineBuffer {
        LineBuffer {
            buf: String::with_capacity(capacity),
            pos: 0,
        }
    }

    #[cfg(test)]
    pub fn init(line: &str, pos: usize) -> LineBuffer {
        let mut lb = Self::with_capacity(MAX_LINE);
        assert!(lb.insert_str(0, line));
        lb.set_pos(pos);
        lb
    }

    /// Extracts a string slice containing the entire buffer.
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    /// Converts a buffer into a `String` without copying or allocating.
    pub fn into_string(self) -> String {
        self.buf
    }

    /// Current cursor position (byte position)
    pub fn pos(&self) -> usize {
        self.pos
    }
    pub fn set_pos(&mut self, pos: usize) {
        assert!(pos <= self.buf.len());
        self.pos = pos;
    }

    /// Returns the length of this buffer, in bytes.
    pub fn len(&self) -> usize {
        self.buf.len()
    }
    /// Returns `true` if this buffer has a length of zero.
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Set line content (`buf`) and cursor position (`pos`).
    pub fn update(&mut self, buf: &str, pos: usize) {
        assert!(pos <= buf.len());
        self.buf.clear();
        let max = self.buf.capacity();
        if buf.len() > max {
            self.buf.push_str(&buf[..max]);
            if pos > max {
                self.pos = max;
            } else {
                self.pos = pos;
            }
        } else {
            self.buf.push_str(buf);
            self.pos = pos;
        }
    }

    /// Backup `src`
    pub fn backup(&mut self, src: &LineBuffer) {
        self.buf.clear();
        self.buf.push_str(&src.buf);
        self.pos = src.pos;
    }

    /// Returns the character at current cursor position.
    fn grapheme_at_cursor(&self) -> Option<&str> {
        if self.pos == self.buf.len() {
            None
        } else {
            self.buf[self.pos..].graphemes(true).next()
        }
    }

    fn next_pos(&self, n: RepeatCount) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .take(n)
            .last()
            .map(|(i, s)| i + self.pos + s.len())
    }
    /// Returns the position of the character just before the current cursor position.
    fn prev_pos(&self, n: RepeatCount) -> Option<usize> {
        if self.pos == 0 {
            return None;
        }
        self.buf[..self.pos]
            .grapheme_indices(true)
            .rev()
            .take(n)
            .last()
            .map(|(i, _)| i)
    }

    /// Insert the character `ch` at current cursor position
    /// and advance cursor position accordingly.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn insert(&mut self, ch: char, n: RepeatCount) -> Option<bool> {
        let shift = ch.len_utf8() * n;
        if self.buf.len() + shift > self.buf.capacity() {
            return None;
        }
        let push = self.pos == self.buf.len();
        if push {
            self.buf.reserve(shift);
            for _ in 0..n {
                self.buf.push(ch);
            }
        } else if n == 1 {
            self.buf.insert(self.pos, ch);
        } else {
            let text = iter::repeat(ch).take(n).collect::<String>();
            let pos = self.pos;
            self.insert_str(pos, &text);
        }
        self.pos += shift;
        Some(push)
    }

    /// Yank/paste `text` at current position.
    /// Return `None` when maximum buffer size has been reached,
    /// `true` when the character has been appended to the end of the line.
    pub fn yank(&mut self, text: &str, n: RepeatCount) -> Option<bool> {
        let shift = text.len() * n;
        if text.is_empty() || (self.buf.len() + shift) > self.buf.capacity() {
            return None;
        }
        let push = self.pos == self.buf.len();
        if push {
            self.buf.reserve(shift);
            for _ in 0..n {
                self.buf.push_str(text);
            }
        } else {
            let text = iter::repeat(text).take(n).collect::<String>();
            let pos = self.pos;
            self.insert_str(pos, &text);
        }
        self.pos += shift;
        Some(push)
    }

    /// Delete previously yanked text and yank/paste `text` at current position.
    pub fn yank_pop(&mut self, yank_size: usize, text: &str) -> Option<bool> {
        self.buf.drain((self.pos - yank_size)..self.pos);
        self.pos -= yank_size;
        self.yank(text, 1)
    }

    /// Move cursor on the left.
    pub fn move_backward(&mut self, n: RepeatCount) -> bool {
        match self.prev_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor on the right.
    pub fn move_forward(&mut self, n: RepeatCount) -> bool {
        match self.next_pos(n) {
            Some(pos) => {
                self.pos = pos;
                true
            }
            None => false,
        }
    }

    /// Move cursor to the start of the line.
    pub fn move_home(&mut self) -> bool {
        if self.pos > 0 {
            self.pos = 0;
            true
        } else {
            false
        }
    }

    /// Move cursor to the end of the line.
    pub fn move_end(&mut self) -> bool {
        if self.pos == self.buf.len() {
            false
        } else {
            self.pos = self.buf.len();
            true
        }
    }

    /// Delete the character at the right of the cursor without altering the cursor
    /// position. Basically this is what happens with the "Delete" keyboard key.
    /// Return the number of characters deleted.
    pub fn delete(&mut self, n: RepeatCount) -> Option<String> {
        match self.next_pos(n) {
            Some(pos) => {
                let chars = self.buf.drain(self.pos..pos).collect::<String>();
                Some(chars)
            }
            None => None,
        }
    }

    /// Delete the character at the left of the cursor.
    /// Basically that is what happens with the "Backspace" keyboard key.
    pub fn backspace(&mut self, n: RepeatCount) -> Option<String> {
        match self.prev_pos(n) {
            Some(pos) => {
                let chars = self.buf.drain(pos..self.pos).collect::<String>();
                self.pos = pos;
                Some(chars)
            }
            None => None,
        }
    }

    /// Kill the text from point to the end of the line.
    pub fn kill_line(&mut self) -> Option<String> {
        if !self.buf.is_empty() && self.pos < self.buf.len() {
            let text = self.buf.drain(self.pos..).collect();
            Some(text)
        } else {
            None
        }
    }

    /// Kill backward from point to the beginning of the line.
    pub fn discard_line(&mut self) -> Option<String> {
        if self.pos > 0 && !self.buf.is_empty() {
            let text = self.buf.drain(..self.pos).collect();
            self.pos = 0;
            Some(text)
        } else {
            None
        }
    }

    /// Exchange the char before cursor with the character at cursor.
    pub fn transpose_chars(&mut self) -> bool {
        if self.pos == 0 || self.buf.graphemes(true).count() < 2 {
            return false;
        }
        if self.pos == self.buf.len() {
            self.move_backward(1);
        }
        let chars = self.delete(1).unwrap();
        self.move_backward(1);
        self.yank(&chars, 1);
        self.move_forward(1);
        true
    }

    /// Go left until start of word
    fn prev_word_pos(&self, pos: usize, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == 0 {
            return None;
        }
        let mut sow = 0;
        let mut gis = self.buf[..pos].grapheme_indices(true).rev();
        'outer: for _ in 0..n {
            let mut gj = gis.next();
            'inner: loop {
                match gj {
                    Some((j, y)) => {
                        let gi = gis.next();
                        match gi {
                            Some((_, x)) => {
                                if is_start_of_word(word_def, x, y) {
                                    sow = j;
                                    break 'inner;
                                }
                                gj = gi;
                            }
                            None => {
                                break 'outer;
                            }
                        }
                    }
                    None => {
                        break 'outer;
                    }
                }
            }
        }
        Some(sow)
    }

    /// Moves the cursor to the beginning of previous word.
    pub fn move_to_prev_word(&mut self, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Delete the previous word, maintaining the cursor at the start of the
    /// current word.
    pub fn delete_prev_word(&mut self, word_def: Word, n: RepeatCount) -> Option<String> {
        if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
            let word = self.buf.drain(pos..self.pos).collect();
            self.pos = pos;
            Some(word)
        } else {
            None
        }
    }

    fn next_word_pos(&self, pos: usize, at: At, word_def: Word, n: RepeatCount) -> Option<usize> {
        if pos == self.buf.len() {
            return None;
        }
        let mut wp = 0;
        let mut gis = self.buf[pos..].grapheme_indices(true);
        let mut gi = if at != At::Start {
            // TODO Validate
            gis.next()
        } else {
            None
        };
        'outer: for _ in 0..n {
            gi = gis.next();
            'inner: loop {
                match gi {
                    Some((i, x)) => {
                        let gj = gis.next();
                        match gj {
                            Some((j, y)) => {
                                if at == At::Start && is_start_of_word(word_def, x, y) {
                                    wp = j;
                                    break 'inner;
                                } else if at != At::Start && is_end_of_word(word_def, x, y) {
                                    if word_def == Word::Emacs || at == At::AfterEnd {
                                        wp = j;
                                    } else {
                                        wp = i;
                                    }
                                    break 'inner;
                                }
                                gi = gj;
                            }
                            None => {
                                break 'outer;
                            }
                        }
                    }
                    None => {
                        break 'outer;
                    }
                }
            }
        }
        if wp == 0 {
            if word_def == Word::Emacs || at == At::AfterEnd {
                Some(self.buf.len())
            } else {
                match gi {
                    Some((i, _)) if i != 0 => Some(i + pos),
                    _ => None,
                }
            }
        } else {
            Some(wp + pos)
        }
    }

    /// Moves the cursor to the end of next word.
    pub fn move_to_next_word(&mut self, at: At, word_def: Word, n: RepeatCount) -> bool {
        if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    fn search_char_pos(&self, cs: &CharSearch, n: RepeatCount) -> Option<usize> {
        let mut shift = 0;
        let search_result = match *cs {
            CharSearch::Backward(c) | CharSearch::BackwardAfter(c) => self.buf[..self.pos]
                .char_indices()
                .rev()
                .filter(|&(_, ch)| ch == c)
                .take(n)
                .last()
                .map(|(i, _)| i),
            CharSearch::Forward(c) | CharSearch::ForwardBefore(c) => {
                if let Some(cc) = self.grapheme_at_cursor() {
                    shift = self.pos + cc.len();
                    if shift < self.buf.len() {
                        self.buf[shift..]
                            .char_indices()
                            .filter(|&(_, ch)| ch == c)
                            .take(n)
                            .last()
                            .map(|(i, _)| i)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        };
        if let Some(pos) = search_result {
            Some(match *cs {
                CharSearch::Backward(_) => pos,
                CharSearch::BackwardAfter(c) => pos + c.len_utf8(),
                CharSearch::Forward(_) => shift + pos,
                CharSearch::ForwardBefore(_) => {
                    shift + pos
                        - self.buf[..shift + pos]
                            .chars()
                            .next_back()
                            .unwrap()
                            .len_utf8()
                }
            })
        } else {
            None
        }
    }

    pub fn move_to(&mut self, cs: CharSearch, n: RepeatCount) -> bool {
        if let Some(pos) = self.search_char_pos(&cs, n) {
            self.pos = pos;
            true
        } else {
            false
        }
    }

    /// Kill from the cursor to the end of the current word,
    /// or, if between words, to the end of the next word.
    pub fn delete_word(&mut self, at: At, word_def: Word, n: RepeatCount) -> Option<String> {
        if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
            let word = self.buf.drain(self.pos..pos).collect();
            Some(word)
        } else {
            None
        }
    }

    pub fn delete_to(&mut self, cs: CharSearch, n: RepeatCount) -> Option<String> {
        let search_result = match cs {
            CharSearch::ForwardBefore(c) => self.search_char_pos(&CharSearch::Forward(c), n),
            _ => self.search_char_pos(&cs, n),
        };
        if let Some(pos) = search_result {
            let chunk = match cs {
                CharSearch::Backward(_) | CharSearch::BackwardAfter(_) => {
                    let end = self.pos;
                    self.pos = pos;
                    self.buf.drain(pos..end).collect()
                }
                CharSearch::ForwardBefore(_) => self.buf.drain(self.pos..pos).collect(),
                CharSearch::Forward(c) => self.buf.drain(self.pos..pos + c.len_utf8()).collect(),
            };
            Some(chunk)
        } else {
            None
        }
    }

    fn skip_whitespace(&self) -> Option<usize> {
        if self.pos == self.buf.len() {
            return None;
        }
        self.buf[self.pos..]
            .grapheme_indices(true)
            .filter(|&(_, ch)| ch.chars().all(|c| c.is_alphanumeric()))
            .map(|(i, _)| i)
            .next()
            .map(|i| i + self.pos)
    }
    /// Alter the next word.
    pub fn edit_word(&mut self, a: WordAction) -> bool {
        if let Some(start) = self.skip_whitespace() {
            if let Some(end) = self.next_word_pos(start, At::AfterEnd, Word::Emacs, 1) {
                if start == end {
                    return false;
                }
                let word = self.buf.drain(start..end).collect::<String>();
                let result = match a {
                    WordAction::CAPITALIZE => {
                        let ch = (&word).graphemes(true).next().unwrap();
                        let cap = ch.to_uppercase();
                        cap + &word[ch.len()..].to_lowercase()
                    }
                    WordAction::LOWERCASE => word.to_lowercase(),
                    WordAction::UPPERCASE => word.to_uppercase(),
                };
                self.insert_str(start, &result);
                self.pos = start + result.len();
                return true;
            }
        }
        false
    }

    /// Transpose two words
    pub fn transpose_words(&mut self, n: RepeatCount) -> bool {
        let word_def = Word::Emacs;
        self.move_to_next_word(At::AfterEnd, word_def, n);
        let w2_end = self.pos;
        self.move_to_prev_word(word_def, 1);
        let w2_beg = self.pos;
        self.move_to_prev_word(word_def, n);
        let w1_beg = self.pos;
        self.move_to_next_word(At::AfterEnd, word_def, 1);
        let w1_end = self.pos;
        if w1_beg == w2_beg || w2_beg < w1_end {
            return false;
        }

        let w1 = self.buf[w1_beg..w1_end].to_string();

        let w2 = self.buf.drain(w2_beg..w2_end).collect::<String>();
        self.insert_str(w2_beg, &w1);

        self.buf.drain(w1_beg..w1_end);
        self.insert_str(w1_beg, &w2);

        self.pos = w2_end;
        true
    }

    /// Replaces the content between [`start`..`end`] with `text`
    /// and positions the cursor to the end of text.
    pub fn replace(&mut self, range: Range<usize>, text: &str) {
        let start = range.start;
        self.buf.drain(range);
        self.insert_str(start, text);
        self.pos = start + text.len();
    }

    fn insert_str(&mut self, idx: usize, s: &str) -> bool {
        if idx == self.buf.len() {
            self.buf.push_str(s);
            true
        } else {
            insert_str(&mut self.buf, idx, s);
            false
        }
    }

    pub fn copy(&self, mvt: Movement) -> Option<String> {
        if self.is_empty() {
            return None;
        }
        match mvt {
            Movement::WholeLine => Some(self.buf.clone()),
            Movement::BeginningOfLine => {
                if self.pos == 0 {
                    None
                } else {
                    Some(self.buf[..self.pos].to_string())
                }
            }
            Movement::ViFirstPrint => {
                if self.pos == 0 {
                    None
                } else if let Some(pos) = self.next_word_pos(0, At::Start, Word::Big, 1) {
                    Some(self.buf[pos..self.pos].to_owned())
                } else {
                    None
                }
            }
            Movement::EndOfLine => {
                if self.pos == self.buf.len() {
                    None
                } else {
                    Some(self.buf[self.pos..].to_string())
                }
            }
            Movement::BackwardWord(n, word_def) => {
                if let Some(pos) = self.prev_word_pos(self.pos, word_def, n) {
                    Some(self.buf[pos..self.pos].to_string())
                } else {
                    None
                }
            }
            Movement::ForwardWord(n, at, word_def) => {
                if let Some(pos) = self.next_word_pos(self.pos, at, word_def, n) {
                    Some(self.buf[self.pos..pos].to_string())
                } else {
                    None
                }
            }
            Movement::ViCharSearch(n, cs) => {
                let search_result = match cs {
                    CharSearch::ForwardBefore(c) => {
                        self.search_char_pos(&CharSearch::Forward(c), n)
                    }
                    _ => self.search_char_pos(&cs, n),
                };
                if let Some(pos) = search_result {
                    Some(match cs {
                        CharSearch::Backward(_) | CharSearch::BackwardAfter(_) => {
                            self.buf[pos..self.pos].to_string()
                        }
                        CharSearch::ForwardBefore(_) => self.buf[self.pos..pos].to_string(),
                        CharSearch::Forward(c) => {
                            self.buf[self.pos..pos + c.len_utf8()].to_string()
                        }
                    })
                } else {
                    None
                }
            }
            Movement::BackwardChar(n) => {
                if let Some(pos) = self.prev_pos(n) {
                    Some(self.buf[pos..self.pos].to_string())
                } else {
                    None
                }
            }
            Movement::ForwardChar(n) => {
                if let Some(pos) = self.next_pos(n) {
                    Some(self.buf[self.pos..pos].to_string())
                } else {
                    None
                }
            }
        }
    }
}

impl Deref for LineBuffer {
    type Target = str;

    fn deref(&self) -> &str {
        self.as_str()
    }
}

fn insert_str(buf: &mut String, idx: usize, s: &str) {
    use std::ptr;

    let len = buf.len();
    assert!(idx <= len);
    assert!(buf.is_char_boundary(idx));
    let amt = s.len();
    buf.reserve(amt);

    unsafe {
        let v = buf.as_mut_vec();
        ptr::copy(
            v.as_ptr().offset(idx as isize),
            v.as_mut_ptr().offset((idx + amt) as isize),
            len - idx,
        );
        ptr::copy_nonoverlapping(s.as_ptr(), v.as_mut_ptr().offset(idx as isize), amt);
        v.set_len(len + amt);
    }
}

fn is_start_of_word(word_def: Word, previous: &str, grapheme: &str) -> bool {
    (!is_word_char(word_def, previous) && is_word_char(word_def, grapheme))
        || (word_def == Word::Vi && !is_other_char(previous) && is_other_char(grapheme))
}
fn is_end_of_word(word_def: Word, grapheme: &str, next: &str) -> bool {
    (!is_word_char(word_def, next) && is_word_char(word_def, grapheme))
        || (word_def == Word::Vi && !is_other_char(next) && is_other_char(grapheme))
}

fn is_word_char(word_def: Word, grapheme: &str) -> bool {
    match word_def {
        Word::Emacs => grapheme.chars().all(|c| c.is_alphanumeric()),
        Word::Vi => is_vi_word_char(grapheme),
        Word::Big => !grapheme.chars().all(|c| c.is_whitespace()),
    }
}
fn is_vi_word_char(grapheme: &str) -> bool {
    grapheme.chars().all(|c| c.is_alphanumeric()) || grapheme == "_"
}
fn is_other_char(grapheme: &str) -> bool {
    !(grapheme.chars().all(|c| c.is_whitespace()) || is_vi_word_char(grapheme))
}
