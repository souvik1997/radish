use super::super::super::Lua;
use super::Engine;
use std::borrow::Cow;

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
    fn completions<'b>(&'b mut self, start: &str, line: &str) -> Option<Vec<(Cow<'b, str>, Cow<'b, str>)>> {
        // TODO
        Some(vec![
            (Cow::Borrowed("test_file"), Cow::Borrowed("test description")),
        ])
    }

    fn category<'b>(&'b self) -> &'b str {
        &self.name
    }
}
