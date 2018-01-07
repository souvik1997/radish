use std::cmp;
use shell::readline::ReadlineEvent;

use super::Buffer;
use super::event::*;
use super::history::HistoryManager;
use shell::history::History;
use shell::completion::Completer;

/// Represents the position of the cursor relative to words in the buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorPosition {
    /// The cursor is in the word with the specified index.
    InWord(usize),

    /// The cursor is on the left edge of the word with the specified index.
    /// For example: `abc |hi`, where `|` is the cursor.
    OnWordLeftEdge(usize),

    /// The cursor is on the right edge of the word with the specified index.
    /// For example: `abc| hi`, where `|` is the cursor.
    OnWordRightEdge(usize),

    /// The cursor is not in contact with any word. Each `Option<usize>` specifies the index of the
    /// closest word to the left and right, respectively, or `None` if there is no word on that side.
    InSpace(Option<usize>, Option<usize>),
}

impl CursorPosition {
    pub fn get(cursor: usize, words: &[(usize, usize)]) -> CursorPosition {
        if words.len() == 0 {
            return CursorPosition::InSpace(None, None);
        } else if cursor == words[0].0 {
            return CursorPosition::OnWordLeftEdge(0);
        } else if cursor < words[0].0 {
            return CursorPosition::InSpace(None, Some(0));
        }

        for (i, &(start, end)) in words.iter().enumerate() {
            if start == cursor {
                return CursorPosition::OnWordLeftEdge(i);
            } else if end == cursor {
                return CursorPosition::OnWordRightEdge(i);
            } else if start < cursor && cursor < end {
                return CursorPosition::InWord(i);
            } else if cursor < start {
                return CursorPosition::InSpace(Some(i - 1), Some(i));
            }
        }
        CursorPosition::InSpace(Some(words.len() - 1), None)
    }
}

/// The core line editor. Displays and provides editing for history and the new buffer.
pub struct Editor<'a, 'b: 'a> {
    // The location of the cursor. Note that the cursor does not lie on a char, but between chars.
    // So, if `cursor == 0` then the cursor is before the first char,
    // and if `cursor == 1` ten the cursor is after the first char and before the second char.
    cursor: usize,

    // Buffer for the new line (ie. not from editing history)
    new_buf: Buffer,

    // None if we're on the new buffer, else the index of history
    cur_history_loc: Option<usize>,

    // If this is true, on the next tab we print the completion list.

    // Show autosuggestions based on history
    show_autosuggestions: bool,

    // if set, the cursor will not be allow to move one past the end of the line, this is necessary
    // for Vi's normal mode.
    pub no_eol: bool,

    pub history: HistoryManager<'a>,
    pub completer: Option<&'a mut Completer<'b>>,
}

macro_rules! cur_buf_mut {
    ($s:expr) => {
        match $s.cur_history_loc {
            Some(i) => &mut $s.history[i],
            _ => &mut $s.new_buf,
        }
    }
}

macro_rules! cur_buf {
    ($s:expr) => {
        match $s.cur_history_loc {
            Some(i) => &$s.history[i],
            _ => &$s.new_buf,
        }
    }
}

impl<'a, 'b: 'a> Editor<'a, 'b> {
    pub fn new(history: Option<&'a History>, completer: Option<&'a mut Completer<'b>>) -> Self {
        Editor::new_with_init_buffer(Buffer::new(), history, completer)
    }

    pub fn new_with_init_buffer<B: Into<Buffer>>(buffer: B, history: Option<&'a History>, completer: Option<&'a mut Completer<'b>>) -> Self {
        let mut ed = Editor {
            cursor: 0,
            new_buf: buffer.into(),
            cur_history_loc: None,
            show_autosuggestions: true,
            no_eol: false,
            history: HistoryManager::new(history),
            completer: completer,
        };

        if !ed.new_buf.is_empty() {
            ed.move_cursor_to_end_of_line();
        }
        ed
    }

    /// None if we're on the new buffer, else the index of history
    pub fn current_history_location(&self) -> Option<usize> {
        self.cur_history_loc
    }

    pub fn get_words_and_cursor_position(&self) -> (Vec<(usize, usize)>, CursorPosition) {
        let words = get_buffer_words(cur_buf!(self));
        let pos = CursorPosition::get(self.cursor, &words);
        (words, pos)
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    // XXX: Returning a bool to indicate doneness is a bit awkward, maybe change it
    pub fn handle_newline(&mut self) -> ReadlineEvent {
        ReadlineEvent::Done
    }

    /// Attempts to undo an action on the current buffer.
    ///
    /// Returns `Ok(true)` if an action was undone.
    /// Returns `Ok(false)` if there was no action to undo.
    pub fn undo(&mut self) -> bool {
        let did = cur_buf_mut!(self).undo();
        if did {
            self.move_cursor_to_end_of_line();
        }
        did
    }

    pub fn redo(&mut self) -> bool {
        let did = cur_buf_mut!(self).redo();
        if did {
            self.move_cursor_to_end_of_line();
        }
        did
    }

    pub fn revert(&mut self) -> bool {
        let did = cur_buf_mut!(self).revert();
        if did {
            self.move_cursor_to_end_of_line();
        }
        did
    }

    pub fn complete(&mut self, handler: &mut EventHandler) -> ReadlineEvent {
        handler(Event::new(self, EventKind::BeforeComplete));

        let (_word, completions) = {
            let word_range = self.get_word_before_cursor(false);
            let buf = cur_buf_mut!(self);

            let word = match word_range {
                Some((start, end)) => buf.range(start, end),
                None => "".into(),
            };

            if let Some(ref mut completer) = self.completer {
                let mut completions = completer.completions(word.as_ref(), &buf.data);
                (word, completions)
            } else {
                return ReadlineEvent::Continue;
            }
        };

        if completions.len() == 0 {
            // Do nothing.
            ReadlineEvent::Continue
        } else if completions.len() == 1 {
            self.delete_word_before_cursor(false);
            self.insert_str_after_cursor(completions.pick_one().unwrap().replacement.as_ref());
            ReadlineEvent::Continue
        } else {
            ReadlineEvent::StartCompletionPager(completions)
        }
    }

    fn get_word_before_cursor(&self, ignore_space_before_cursor: bool) -> Option<(usize, usize)> {
        let (words, pos) = self.get_words_and_cursor_position();
        match pos {
            CursorPosition::InWord(i) => Some(words[i]),
            CursorPosition::InSpace(Some(i), _) => if ignore_space_before_cursor {
                Some(words[i])
            } else {
                None
            },
            CursorPosition::InSpace(None, _) => None,
            CursorPosition::OnWordLeftEdge(i) => if ignore_space_before_cursor && i > 0 {
                Some(words[i - 1])
            } else {
                None
            },
            CursorPosition::OnWordRightEdge(i) => Some(words[i]),
        }
    }

    /// Deletes the word preceding the cursor.
    /// If `ignore_space_before_cursor` is true and there is space directly before the cursor,
    /// this method ignores that space until it finds a word.
    /// If `ignore_space_before_cursor` is false and there is space directly before the cursor,
    /// nothing is deleted.
    pub fn delete_word_before_cursor(&mut self, ignore_space_before_cursor: bool) -> ReadlineEvent {
        if let Some((start, _)) = self.get_word_before_cursor(ignore_space_before_cursor) {
            let moved = cur_buf_mut!(self).remove(start, self.cursor);
            self.cursor -= moved;
        }
        ReadlineEvent::Continue
    }

    /// Clears the screen then prints the prompt and current buffer.
    pub fn clear(&mut self) -> ReadlineEvent {
        ReadlineEvent::ClearScreen
    }

    /// Move up (backwards) in history.
    pub fn move_up(&mut self) -> ReadlineEvent {
        if let Some(i) = self.cur_history_loc {
            if i > 0 {
                self.cur_history_loc = Some(i - 1);
            } else {
                return ReadlineEvent::Continue;
            }
        } else {
            if self.history.len() > 0 {
                self.cur_history_loc = Some(self.history.len() - 1);
            } else {
                return ReadlineEvent::Continue;
            }
        }

        self.move_cursor_to_end_of_line()
    }

    /// Move down (forwards) in history, or to the new buffer if we reach the end of history.
    pub fn move_down(&mut self) -> ReadlineEvent {
        if let Some(i) = self.cur_history_loc {
            if i < self.history.len() - 1 {
                self.cur_history_loc = Some(i + 1);
            } else {
                self.cur_history_loc = None;
            }
            self.move_cursor_to_end_of_line()
        } else {
            ReadlineEvent::Continue
        }
    }

    /// Moves to the start of history (ie. the earliest history entry).
    pub fn move_to_start_of_history(&mut self) -> ReadlineEvent {
        if self.history.len() > 0 {
            self.cur_history_loc = Some(0);
            self.move_cursor_to_end_of_line()
        } else {
            self.cur_history_loc = None;
            ReadlineEvent::Continue
        }
    }

    /// Moves to the end of history (ie. the new buffer).
    pub fn move_to_end_of_history(&mut self) -> ReadlineEvent {
        if self.cur_history_loc.is_some() {
            self.cur_history_loc = None;
            self.move_cursor_to_end_of_line()
        } else {
            ReadlineEvent::Continue
        }
    }

    /// Inserts a string directly after the cursor, moving the cursor to the right.
    ///
    /// Note: it is more efficient to call `insert_chars_after_cursor()` directly.
    pub fn insert_str_after_cursor(&mut self, s: &str) -> ReadlineEvent {
        self.insert_chars_after_cursor(&s.chars().collect::<Vec<char>>()[..])
    }

    /// Inserts a character directly after the cursor, moving the cursor to the right.
    pub fn insert_after_cursor(&mut self, c: char) -> ReadlineEvent {
        self.insert_chars_after_cursor(&[c])
    }

    /// Inserts characters directly after the cursor, moving the cursor to the right.
    pub fn insert_chars_after_cursor(&mut self, cs: &[char]) -> ReadlineEvent {
        {
            let buf = cur_buf_mut!(self);
            buf.insert(self.cursor, cs);
        }

        self.cursor += cs.len();
        ReadlineEvent::Continue
    }

    /// Deletes the character directly before the cursor, moving the cursor to the left.
    /// If the cursor is at the start of the line, nothing happens.
    pub fn delete_before_cursor(&mut self) -> ReadlineEvent {
        if self.cursor > 0 {
            let buf = cur_buf_mut!(self);
            buf.remove(self.cursor - 1, self.cursor);
            self.cursor -= 1;
        }

        ReadlineEvent::Continue
    }

    /// Deletes the character directly after the cursor. The cursor does not move.
    /// If the cursor is at the end of the line, nothing happens.
    pub fn delete_after_cursor(&mut self) -> ReadlineEvent {
        {
            let buf = cur_buf_mut!(self);

            if self.cursor < buf.num_chars() {
                buf.remove(self.cursor, self.cursor + 1);
            }
        }
        ReadlineEvent::Continue
    }

    /// Deletes every character preceding the cursor until the beginning of the line.
    pub fn delete_all_before_cursor(&mut self) -> ReadlineEvent {
        cur_buf_mut!(self).remove(0, self.cursor);
        self.cursor = 0;
        ReadlineEvent::Continue
    }

    /// Deletes every character after the cursor until the end of the line.
    pub fn delete_all_after_cursor(&mut self) -> ReadlineEvent {
        {
            let buf = cur_buf_mut!(self);
            buf.truncate(self.cursor);
        }
        ReadlineEvent::Continue
    }

    /// Deletes every character from the cursor until the given position.
    pub fn delete_until(&mut self, position: usize) -> ReadlineEvent {
        {
            let buf = cur_buf_mut!(self);
            buf.remove(
                cmp::min(self.cursor, position),
                cmp::max(self.cursor, position),
            );
            self.cursor = cmp::min(self.cursor, position);
        }
        ReadlineEvent::Continue
    }

    /// Deletes every character from the cursor until the given position, inclusive.
    pub fn delete_until_inclusive(&mut self, position: usize) -> ReadlineEvent {
        {
            let buf = cur_buf_mut!(self);
            buf.remove(
                cmp::min(self.cursor, position),
                cmp::max(self.cursor + 1, position + 1),
            );
            self.cursor = cmp::min(self.cursor, position);
        }
        ReadlineEvent::Continue
    }

    /// Moves the cursor to the left by `count` characters.
    /// The cursor will not go past the start of the buffer.
    pub fn move_cursor_left(&mut self, mut count: usize) -> ReadlineEvent {
        if count > self.cursor {
            count = self.cursor;
        }

        self.cursor -= count;

        ReadlineEvent::Continue
    }

    /// Moves the cursor to the right by `count` characters.
    /// The cursor will not go past the end of the buffer.
    pub fn move_cursor_right(&mut self, mut count: usize) -> ReadlineEvent {
        {
            let buf = cur_buf!(self);

            if count > buf.num_chars() - self.cursor {
                count = buf.num_chars() - self.cursor;
            }

            self.cursor += count;
        }

        ReadlineEvent::Continue
    }

    /// Moves the cursor to `pos`. If `pos` is past the end of the buffer, it will be clamped.
    pub fn move_cursor_to(&mut self, pos: usize) -> ReadlineEvent {
        self.cursor = pos;
        let buf_len = cur_buf!(self).num_chars();
        if self.cursor > buf_len {
            self.cursor = buf_len;
        }
        ReadlineEvent::Continue
    }

    /// Moves the cursor to the start of the line.
    pub fn move_cursor_to_start_of_line(&mut self) -> ReadlineEvent {
        self.cursor = 0;
        ReadlineEvent::Continue
    }

    /// Moves the cursor to the end of the line.
    pub fn move_cursor_to_end_of_line(&mut self) -> ReadlineEvent {
        self.cursor = cur_buf!(self).num_chars();
        ReadlineEvent::Continue
    }

    pub fn cursor_is_at_end_of_line(&self) -> bool {
        let num_chars = cur_buf!(self).num_chars();
        if self.no_eol {
            self.cursor == num_chars - 1
        } else {
            self.cursor == num_chars
        }
    }

    ///  Returns a reference to the current buffer being edited.
    /// This may be the new buffer or a buffer from history.
    pub fn current_buffer(&self) -> &Buffer {
        cur_buf!(self)
    }

    ///  Returns a mutable reference to the current buffer being edited.
    /// This may be the new buffer or a buffer from history.
    pub fn current_buffer_mut(&mut self) -> &mut Buffer {
        cur_buf_mut!(self)
    }

    /// Accept autosuggestion and copy its content into current buffer
    pub fn accept_autosuggestion(&mut self) -> ReadlineEvent {
        if self.show_autosuggestions {
            {
                let autosuggestion = self.current_autosuggestion().cloned();
                let buf = self.current_buffer_mut();
                if let Some(x) = autosuggestion {
                    buf.insert_from_buffer(&x);
                }
            }
        }
        self.move_cursor_to_end_of_line()
    }

    pub fn current_autosuggestion(&self) -> Option<&Buffer> {
        if self.show_autosuggestions {
            self.history
                .get_newest_match(self.cur_history_loc, self.current_buffer())
        } else {
            None
        }
    }

    pub fn is_currently_showing_autosuggestion(&self) -> bool {
        self.current_autosuggestion().is_some()
    }
}

impl<'a, 'b: 'a> From<Editor<'a, 'b>> for String {
    fn from(ed: Editor<'a, 'b>) -> String {
        match ed.cur_history_loc {
            Some(i) => ed.history[i].clone(),
            _ => ed.new_buf,
        }.into()
    }
}

pub fn get_buffer_words(buf: &Buffer) -> Vec<(usize, usize)> {
    let mut res = Vec::new();

    let mut word_start = None;
    let mut just_had_backslash = false;

    for (i, &c) in buf.chars().enumerate() {
        if c == '\\' {
            just_had_backslash = true;
            continue;
        }

        if let Some(start) = word_start {
            if c == ' ' && !just_had_backslash {
                res.push((start, i));
                word_start = None;
            }
        } else {
            if c != ' ' {
                word_start = Some(i);
            }
        }

        just_had_backslash = false;
    }

    if let Some(start) = word_start {
        res.push((start, buf.num_chars()));
    }

    res
}
