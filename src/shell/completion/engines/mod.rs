pub mod path_completion;
pub use self::path_completion::*;

pub mod user_completion;
pub use self::user_completion::*;

use std::borrow::Cow;

pub trait Engine {
    fn completions<'a>(&'a mut self, start: &str, line: &str) -> Option<Vec<(Cow<'a, str>, Cow<'a, str>)>>;
    fn category<'a>(&'a self) -> &'a str;
}
