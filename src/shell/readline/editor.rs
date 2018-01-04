use super::display::*;
use super::termion;
use super::line_editor::LineEditor;
use super::ReadlineEvent;
use super::super::history::History;
use super::super::completion::Completer;
use self::termion::event::Key;


pub struct Editor<'a> {
    line_editor: LineEditor<'a>,
    completer: &'a Completer<'a>,
    history: &'a History,
}

impl<'a> Editor<'a> {
    pub fn new(completer: &'a Completer<'a>, history: &'a History) -> Editor<'a> {
        Editor {
            line_editor: LineEditor::new(DisplayString::from("$ prompt "), completer, history),
            completer: completer,
            history: history,
        }
    }
    pub fn handle_input(&mut self, key: Key) -> ReadlineEvent {
        self.line_editor.handle_input(key)
    }

    pub fn buffer(&self) -> String {
        self.line_editor.buffer().iter().cloned().collect::<String>()
    }

}


impl<'a> Render for Editor<'a> {
    fn render<F: FnMut(&Row)>(&mut self, render_fn: &mut F, width: usize) {
        self.line_editor.render(render_fn, width);
    }
}
