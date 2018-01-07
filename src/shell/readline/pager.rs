use super::display::*;
use super::termion;
use super::super::completion::Completer;
use super::super::history::History;
use self::unicode_width::UnicodeWidthStr;


pub enum PagerResult {
    Selected(String),
    Abort,
    Continue,
}

pub struct Pager<'a> {
    completer: &'a Completer<'a>,
}

impl<'a> Pager<'a> {
    pub fn new(&'a Completer<'a>) -> Pager<'a> {
        Pager {
            completer: completer
        }
    }

    pub fn handle_input(&mut self, key: termion::event::Key) -> ReadlineEvent {
        self.editor.handle_key(key, &mut |_| {})
    }
}
