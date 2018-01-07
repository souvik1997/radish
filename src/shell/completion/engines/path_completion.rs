use super::Engine;

pub struct PathCompletion {}

impl PathCompletion {
    pub fn new() -> PathCompletion {
        PathCompletion {}
    }
}

impl Engine for PathCompletion {
    fn completions(&mut self, start: &str, line: &str) -> Option<Vec<(String, String)>> {
        // TODO
        Some(vec![
            (String::from("test_file"), String::from("test description")),
        ])
    }

    fn category<'a>(&'a self) -> &'a str {
        "Files"
    }
}
