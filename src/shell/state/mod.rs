use super::readline;

pub struct Job {
    child_pid: u32
}

pub struct ShellState {
    jobs: Vec<Job>,
}

impl ShellState {
    pub fn new() -> Self {
        ShellState {
            jobs: Vec::<Job>::new()
        }
    }
}

impl readline::completion::Completer for ShellState {
    fn complete(&self, _line: &str, _pos: usize) -> readline::Result<(usize, Vec<String>)> {
        Ok((0, Vec::new()))
    }
}
