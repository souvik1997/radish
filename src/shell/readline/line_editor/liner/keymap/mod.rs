use shell::readline::termion::event::Key;
use shell::readline::ReadlineEvent;
use super::Editor;
use super::event::*;

pub trait KeyMap<'a, T>: From<T> {
    fn handle_key_core(&mut self, key: Key) -> ReadlineEvent;
    fn editor(&self) -> &Editor<'a>;
    fn editor_mut(&mut self) -> &mut Editor<'a>;

    fn handle_key(&mut self, mut key: Key, handler: &mut EventHandler) -> ReadlineEvent {
        let mut done = false;

        handler(Event::new(self.editor_mut(), EventKind::BeforeKey(key)));

        let is_empty = self.editor().current_buffer().is_empty();

        if key == Key::Ctrl('h') {
            // XXX: Might need to change this when remappable keybindings are added.
            key = Key::Backspace;
        }

        let res = match key {
            Key::Ctrl('c') => {
                return ReadlineEvent::Interrupted;
            }
            // if the current buffer is empty, treat ctrl-d as eof
            Key::Ctrl('d') if is_empty => {
                return ReadlineEvent::Eof;
            }
            Key::Char('\t') => self.editor_mut().complete(handler),
            Key::Char('\n') => {
                return ReadlineEvent::Done;
            }
            Key::Ctrl('f') if self.editor().is_currently_showing_autosuggestion() => {
                self.editor_mut().accept_autosuggestion()
            }
            Key::Right if self.editor().is_currently_showing_autosuggestion() &&
                          self.editor().cursor_is_at_end_of_line() => {
                self.editor_mut().accept_autosuggestion()
            }
            _ => {
                let res = self.handle_key_core(key);
                self.editor_mut().skip_completions_hint();
                res
            }
        };

        handler(Event::new(self.editor_mut(), EventKind::AfterKey(key)));

        res
    }
}

//pub mod vi;
//pub use self::vi::Vi;

pub mod emacs;
pub use self::emacs::Emacs;
