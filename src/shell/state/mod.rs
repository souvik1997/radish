use super::readline;
use super::jobs::Job;


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

impl readline::delegate::Delegate for ShellState {
    fn complete(&self, _line: &str, _pos: usize) -> readline::Result<(usize, Vec<String>)> {
        Ok((0, Vec::new()))
    }

    fn prompt(&self) -> String {
        String::from("$ ")
    }
}
