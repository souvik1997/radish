use super::super::super::Lua;
use super::Engine;

pub struct UserCompletion<'a> {
    lua: &'a Lua,
    name: String,
}

impl<'a> UserCompletion<'a> {
    pub fn new(lua: &'a Lua, name: &str) -> UserCompletion<'a> {
        UserCompletion { lua: lua, name: name.to_owned() }
    }
}

impl<'a> Engine for UserCompletion<'a> {
    fn completions(&mut self, start: &str, line: &str) -> Option<Vec<(String, String)>> {
        // TODO
        Some(vec![
            (String::from("test_file"), String::from("test description")),
        ])
    }

    fn category<'b>(&'b self) -> &'b str {
        &self.name
    }
}
