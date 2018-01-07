use shell::readline::termion::event::Key;
use super::Editor;

pub type EventHandler<'a> = FnMut(Event) + 'a;

pub struct Event<'a, 'b: 'a> {
    pub editor: &'a mut Editor<'b>,
    pub kind: EventKind,
}

impl<'a, 'b: 'a> Event<'a, 'b> {
    pub fn new(editor: &'a mut Editor<'b>, kind: EventKind) -> Self {
        Event {
            editor: editor,
            kind: kind,
        }
    }
}

#[derive(Debug)]
pub enum EventKind {
    /// Sent before handling a keypress.
    BeforeKey(Key),
    /// Sent after handling a keypress.
    AfterKey(Key),
    /// Sent in `Editor.complete()`, before processing the completion.
    BeforeComplete,
}
