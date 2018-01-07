pub mod path_completion;
pub use self::path_completion::*;

pub mod user_completion;
pub use self::user_completion::*;

pub trait Engine {
    fn completions(&mut self, start: &str, line: &str) -> Option<Vec<(String, String)>>;
    fn category<'a>(&'a self) -> &'a str;
}
