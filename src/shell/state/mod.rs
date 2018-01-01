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
use std::ops::DerefMut;
use std::process;


pub struct ShellState {
    background_jobs: Vec<jobs::Job>,
    current_job: Option<Rc<jobs::Job>>,
    pub ketos_interp: Interpreter,
    builtins: HashMap<String, Box<(FnMut(&[String]) -> i8)>>
}

impl ShellState {
    pub fn new() -> Self {
        let mut s = ShellState {
            background_jobs: Vec::<jobs::Job>::new(),
            current_job: None,
            ketos_interp: Interpreter::new(),
            builtins: HashMap::new()
        };
        s.builtins.insert(String::from("cd"), Box::new(|args: &[String]| -> i8 {
            if let Some(first) = args.first() {
                let p = PathBuf::from(first);
                if p.exists() && p.is_dir() {
                    match env::set_current_dir(p) {
                        Ok(_) => 0,
                        Err(_) => -1,
                    }
                } else {
                    1
                }
            } else {
                1
            }
        }));
        s.builtins.insert(String::from("echo"), Box::new(|args: &[String]| -> i8 {
            println!("{}", args.join(" "));
            0
        }));
        s.builtins.insert(String::from("echo-stderr"), Box::new(|args: &[String]| -> i8 {
            eprintln!("{}", args.join(" "));
            0
        }));
        s.builtins.insert(String::from("exit"), Box::new(|_args: &[String]| -> i8 {
            process::exit(0);
        }));
        s
    }

    pub fn run_job(&mut self, expr: &Expr) -> Result<Rc<jobs::Job>, jobs::Error> {
        match jobs::Job::from_expr(&expr, self) {
            Ok(mut job) => {
                job.run(self);
                let result = Rc::new(job);
                self.current_job = Some(result.clone());
                Ok(result)
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

impl jobs::BuiltinHandler for ShellState {
    fn handle_builtin(&mut self, name: &str, args: &[String]) -> i8 {
        if let Some(b) = self.builtins.get_mut(name) {
            let func = Box::deref_mut(b);
            func(args)
        } else {
            let ketos_name = self.ketos_interp.scope().borrow_names_mut().add(name);
            if let Some(value) = self.ketos_interp.scope().get_value(ketos_name) {
                let result = self.ketos_interp.call_value(value, args.into_iter().map(|s| { ketos::Value::String(ketos::rc_vec::RcString::new(s.clone())) }).collect());
                match result {
                    Ok(val) => {
                        self.ketos_interp.display_value(&val);
                        0
                    },
                    Err(error) => {
                        println!("error: {:?}", error);
                        -1
                    }
                }
            } else {
                -1
            }
        }
    }

    fn is_builtin(&mut self, name: &str) -> bool {
        let ketos_name = self.ketos_interp.scope().borrow_names_mut().add(name);
        if let Some(_) = self.ketos_interp.scope().get_value(ketos_name) {
            true
        } else {
            self.builtins.contains_key(name)
        }
    }
}
