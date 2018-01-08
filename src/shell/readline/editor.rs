use super::display::*;
use super::termion;
use super::line_editor::LineEditor;
//use super::pager::{Pager, PagerResult};
use super::ReadlineEvent;
use super::super::history::History;
use super::super::completion::Completer;
use self::termion::event::Key;

pub struct Editor<'a, 'b: 'a> {
    line_editor: LineEditor<'a, 'b>,
    history: &'a History,
}

impl<'a, 'b: 'a> Editor<'a, 'b> {
    pub fn new(completer: &'a mut Completer<'b>, history: &'a History) -> Editor<'a, 'b> {
        Editor {
            line_editor: LineEditor::new(DisplayString::from("$ prompt "), completer, history),
            history: history,
        }
    }
    pub fn handle_input(&mut self, key: Key) -> ReadlineEvent {
        self.line_editor.handle_input(key)
    }

    pub fn buffer(&self) -> String {
        self.line_editor
            .buffer()
            .iter()
            .cloned()
            .collect::<String>()
    }
}

impl<'a, 'b: 'a> Render for Editor<'a, 'b> {
    fn render<F: FnMut(&Row)>(&mut self, render_fn: &mut F, width: usize) {
        self.line_editor.render(render_fn, width);
    }
}
