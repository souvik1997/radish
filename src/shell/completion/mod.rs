use super::history::History;
pub struct Completer<'a> {
    history: &'a History,
}

impl<'a> Completer<'a> {
    pub fn new(history: &'a History) -> Completer<'a> {
        Completer {
            history: history
        }
    }
    pub fn completions(&self, start: &str, line: &[char]) -> Vec<String> {
        Vec::new()
    }
}
