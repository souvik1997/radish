use super::readline;
use super::syntax::ast::Expr;
use std::env;
extern crate users;
extern crate ansi_term;
use self::ansi_term::Colour;
extern crate ketos;
use ketos::Interpreter;
pub mod jobs;
use std::cell::Cell;
use std::path::PathBuf;
use std::collections::HashMap;
use std::rc::Rc;


pub struct ShellState<'a> {
    jobs: Vec<jobs::Job<'a>>,
    pub ketos_interp: Interpreter,
    builtins: HashMap<String, Rc<(FnMut(&[&str], &HashMap<u32, jobs::FdOptions>) -> i32)>>
}

impl<'a> ShellState<'a> {
    pub fn new() -> Self {
        let mut s = ShellState {
            jobs: Vec::<jobs::Job>::new(),
            ketos_interp: Interpreter::new(),
            builtins: HashMap::new()
        };
        s.builtins.insert(String::from("cd"), Rc::new(|args: &[&str], _| -> i32 {
            if let Some(first) = args.first() {
                let p = PathBuf::from(first);
                if p.exists() && p.is_dir() {
                    env::set_current_dir(p);
                    0
                } else {
                    1
                }
            } else {
                1
            }
        }));
        s
    }

    pub fn new_job(&mut self, expr: &Expr) -> Result<jobs::Job, jobs::Error>{
        let builtins = &self.builtins;
        match jobs::Job::from_expr(&expr, &|name| {
            if let Some(b) = builtins.get(name) {
                Some(b.as_ref())
            } else {
                None
            }
        }) {
            Ok(job) => {
                Ok(job)
            },
            Err(e) => Err(e)
        }
    }
    pub fn readline(&mut self) -> readline::Result<String> {
        readline::Editor::new().readline(self)
    }
}

impl<'a> readline::delegate::Delegate for ShellState<'a> {
    fn complete(&self, _line: &str, _pos: usize) -> readline::Result<(usize, Vec<String>)> {
        Ok((0, Vec::new()))
    }

    fn prompt(&self, color: bool) -> String {
        let cwd = {
            match env::current_dir() {
                Ok(x) => String::from(x.to_str().unwrap_or("(none)")),
                Err(e) => format!("(error: {:?})", e)
            }
        };
        let username = users::get_current_username().unwrap_or(String::from("(none)"));
        let last_character = if users::get_current_uid() == 0 { "#" } else { "$" };
        String::from(format!("{username}@{cwd}{last_character} ",
                             username=Colour::Red.normal().paint(username).to_string(),
                             cwd=Colour::Green.normal().paint(cwd).to_string(),
                             last_character=last_character))
    }
}
