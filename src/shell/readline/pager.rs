use super::display::*;
use super::termion;
use super::super::completion::Completions;
use super::super::history::History;
use self::unicode_width::UnicodeWidthStr;


pub enum PagerResult {
    Selected(String),
    Abort,
    Continue,
}

pub struct Pager<'a, 'b: 'a> {
    completions: Completions,
    line_editor: LineEditor<'a, 'b>
}

impl<'a, 'b: 'a> Pager<'a, 'b> {
    pub fn new(&'a Completer<'a>) -> Pager<'a, 'b> {
        Pager {
            completer: completer
        }
    }

    pub fn handle_input(&mut self, key: termion::event::Key) -> ReadlineEvent {

    }
}
