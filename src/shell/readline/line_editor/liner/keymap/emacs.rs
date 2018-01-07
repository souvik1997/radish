use shell::readline::termion::event::Key;
use shell::readline::ReadlineEvent;

use super::KeyMap;
use super::super::Editor;
use super::super::CursorPosition;

/// Emacs keybindings for `Editor`. This is the default for `Context::read_line()`.
///
/// ```
/// use liner::*;
/// let mut context = Context::new();
/// context.key_bindings = KeyBindings::Emacs;
/// ```
pub struct Emacs<'a, 'b: 'a> {
    ed: Editor<'a, 'b>,
    last_arg_fetch_index: Option<usize>,
}

impl<'a, 'b: 'a> Emacs<'a, 'b> {
    pub fn new(ed: Editor<'a, 'b>) -> Self {
        Emacs {
            ed,
            last_arg_fetch_index: None,
        }
    }

    fn handle_ctrl_key(&mut self, c: char) -> ReadlineEvent {
        match c {
            'l' => self.ed.clear(),
            'a' => self.ed.move_cursor_to_start_of_line(),
            'e' => self.ed.move_cursor_to_end_of_line(),
            'b' => self.ed.move_cursor_left(1),
            'f' => self.ed.move_cursor_right(1),
            'd' => self.ed.delete_after_cursor(),
            'p' => self.ed.move_up(),
            'n' => self.ed.move_down(),
            'u' => self.ed.delete_all_before_cursor(),
            'k' => self.ed.delete_all_after_cursor(),
            'w' => self.ed.delete_word_before_cursor(true),
            'x' => {
                self.ed.undo();
                ReadlineEvent::Continue
            }
            _ => ReadlineEvent::Continue,
        }
    }

    fn handle_alt_key(&mut self, c: char) -> ReadlineEvent {
        match c {
            '<' => self.ed.move_to_start_of_history(),
            '>' => self.ed.move_to_end_of_history(),
            '\x7F' => self.ed.delete_word_before_cursor(true),
            'f' => emacs_move_word(&mut self.ed, EmacsMoveDir::Right),
            'b' => emacs_move_word(&mut self.ed, EmacsMoveDir::Left),
            'r' => {
                self.ed.revert();
                ReadlineEvent::Continue
            }
            '.' => self.handle_last_arg_fetch(),
            _ => ReadlineEvent::Continue,
        }
    }

    fn handle_last_arg_fetch(&mut self) -> ReadlineEvent {
        // Empty history means no last arg to fetch.
        if self.ed.history.len() == 0 {
            return ReadlineEvent::Continue;
        }

        let history_index = match self.last_arg_fetch_index {
            Some(0) => return ReadlineEvent::Continue,
            Some(x) => x - 1,
            None => self.ed
                .current_history_location()
                .unwrap_or(self.ed.history.len() - 1),
        };

        // If did a last arg fetch just before this, we need to delete it so it can be replaced by
        // this last arg fetch.
        if self.last_arg_fetch_index.is_some() {
            let buffer_len = self.ed.current_buffer().num_chars();
            if let Some(last_arg_len) = self.ed.current_buffer().last_arg().map(|x| x.len()) {
                self.ed.delete_until(buffer_len - last_arg_len);
            }
        }

        // Actually insert it
        let buf = self.ed.history[history_index].clone();
        if let Some(last_arg) = buf.last_arg() {
            self.ed.insert_chars_after_cursor(last_arg);
        }

        // Edit the index in case the user does a last arg fetch again.
        self.last_arg_fetch_index = Some(history_index);

        ReadlineEvent::Continue
    }
}

impl<'a, 'b: 'a> KeyMap<'a, 'b, Emacs<'a, 'b>> for Emacs<'a, 'b> {
    fn handle_key_core(&mut self, key: Key) -> ReadlineEvent {
        match key {
            Key::Alt('.') => {}
            _ => self.last_arg_fetch_index = None,
        }

        match key {
            Key::Char(c) => self.ed.insert_after_cursor(c),
            Key::Alt(c) => self.handle_alt_key(c),
            Key::Ctrl(c) => self.handle_ctrl_key(c),
            Key::Left => self.ed.move_cursor_left(1),
            Key::Right => self.ed.move_cursor_right(1),
            Key::Up => self.ed.move_up(),
            Key::Down => self.ed.move_down(),
            Key::Home => self.ed.move_cursor_to_start_of_line(),
            Key::End => self.ed.move_cursor_to_end_of_line(),
            Key::Backspace => self.ed.delete_before_cursor(),
            Key::Delete => self.ed.delete_after_cursor(),
            Key::Null => ReadlineEvent::Continue,
            _ => ReadlineEvent::Continue,
        }
    }

    fn editor_mut(&mut self) -> &mut Editor<'a, 'b> {
        &mut self.ed
    }

    fn editor(&self) -> &Editor<'a, 'b> {
        &self.ed
    }
}

impl<'a, 'b: 'a> From<Emacs<'a, 'b>> for String {
    fn from(emacs: Emacs<'a, 'b>) -> String {
        emacs.ed.into()
    }
}

#[derive(PartialEq, Clone, Copy)]
enum EmacsMoveDir {
    Left,
    Right,
}

fn emacs_move_word<'a, 'b: 'a>(ed: &mut Editor<'a, 'b>, direction: EmacsMoveDir) -> ReadlineEvent {
    let (words, pos) = ed.get_words_and_cursor_position();

    let word_index = match pos {
        CursorPosition::InWord(i) => Some(i),
        CursorPosition::OnWordLeftEdge(mut i) => {
            if i > 0 && direction == EmacsMoveDir::Left {
                i -= 1;
            }
            Some(i)
        }
        CursorPosition::OnWordRightEdge(mut i) => {
            if i < words.len() - 1 && direction == EmacsMoveDir::Right {
                i += 1;
            }
            Some(i)
        }
        CursorPosition::InSpace(left, right) => match direction {
            EmacsMoveDir::Left => left,
            EmacsMoveDir::Right => right,
        },
    };

    match word_index {
        None => ReadlineEvent::Continue,
        Some(i) => {
            let (start, end) = words[i];

            let new_cursor_pos = match direction {
                EmacsMoveDir::Left => start,
                EmacsMoveDir::Right => end,
            };

            ed.move_cursor_to(new_cursor_pos)
        }
    }
}
