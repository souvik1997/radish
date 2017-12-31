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


pub struct ShellState {
    jobs: Vec<jobs::Job>,
    pub ketos_interp: Interpreter,
    builtins: HashMap<String, Rc<(FnMut(&[&str], &HashMap<u32, jobs::FdOptions>) -> i32)>>
}

impl ShellState {
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

    pub fn new_job<'a>(&'a mut self, expr: &Expr) -> Result<&'a jobs::Job, jobs::Error>{
        match jobs::Job::from_expr(&expr, &|name| {
            let ketos_name = self.ketos_interp.scope().borrow_names_mut().add(name);
            if let Some(_) = self.ketos_interp.scope().get_value(ketos_name) {
                true
            } else {
                self.builtins.contains_key(name)
            }
        }) {
            Ok(job) => {
                self.jobs.push(job);
                Ok(self.jobs.last().unwrap())
            },
            Err(e) => Err(e)
        }
    }
    pub fn readline(&mut self) -> readline::Result<String> {
        readline::Editor::new().readline(self)
    }
}

impl readline::delegate::Delegate for ShellState {
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
