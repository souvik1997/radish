use std::process;
use nix;
use std::ffi::CString;
use super::super::syntax::ast::{Expr, Argument};
use super::super::syntax::tokens::StringLiteralComponent;
use std::collections::HashMap;
use std::rc::Rc;
use std::path::PathBuf;
use std::env;
use std::fmt;

#[derive(Debug)]
pub enum FdOptions {
    Append(PathBuf),
    Overwrite(PathBuf),
    Input(PathBuf),
    Fd(u32),
}

#[derive(Debug)]
pub enum Configuration {
    Command(PathBuf, Vec<String>, HashMap<u32, FdOptions>),
    Builtin(String, Vec<String>, HashMap<u32, FdOptions>),
    Pipeline(Rc<Job>, Rc<Job>)
}

#[derive(Debug)]
pub enum Status {
    NotStarted,
    Running(u32),
    Done
}

#[derive(Debug)]
pub enum Error {
    Fork,
    Subshell(Rc<Error>),
    CommandNotFound(PathBuf),
    CorruptPath,
    LeftPipe(Rc<Error>),
    RightPipe(Rc<Error>),
}

#[derive(Debug)]
pub struct Job {
    status: Status,
    output: Option<String>,
    configuration: Configuration,
    background: bool
}

impl Job {
    pub fn from_expr<F>(expr: &Expr, is_builtin: &F) -> Result<Job, Error> where F: (Fn(&str) -> bool) {
        match expr {
            &Expr::Command(binary, ref arguments) => {
                let mut fd_options = HashMap::<u32, FdOptions>::new();
                let mut background = false;
                let mut str_arguments = Vec::<String>::new();
                for arg in arguments {
                    match arg {
                        &Argument::Redirect(fd, path) => {
                            fd_options.insert(fd, FdOptions::Overwrite(PathBuf::from(join_components(path))));
                        },
                        &Argument::RedirectFD(fd, target) => { fd_options.insert(fd, FdOptions::Fd(target)); },
                        &Argument::Append(fd, path) => { fd_options.insert(fd, FdOptions::Append(PathBuf::from(join_components(path)))); },
                        &Argument::Input(fd, path) => { fd_options.insert(fd, FdOptions::Input(PathBuf::from(join_components(path)))); },
                        &Argument::Background => { background = true; },
                        &Argument::Subshell(ref subexpr) => {
                            match Job::from_expr(&subexpr, is_builtin) {
                                Ok(mut subjob) => {
                                    match subjob.run_with_output() {
                                        Ok(output) => {
                                            str_arguments.push(output);
                                        }
                                        Err(error) => {
                                            return Err(Error::Subshell(Rc::new(error)));
                                        }
                                    }
                                },
                                Err(error) => {
                                    return Err(Error::Subshell(Rc::new(error)));
                                }
                            }
                        },
                        &Argument::Literal(s) => {
                            str_arguments.push(String::from(join_components(s)));
                        }
                    };
                }
                let binary_str = join_components(binary);
                if is_builtin(&binary_str) {
                    Ok(Job {
                        status: Status::NotStarted,
                        output: None,
                        configuration: Configuration::Builtin(binary_str, str_arguments, fd_options),
                        background: background
                    })
                } else if let Some(path_os_str) = env::var_os("PATH") {
                    let binary_path = PathBuf::from(&binary_str);
                    if let Some(binary_appended_path) = env::split_paths(&path_os_str).filter_map(|mut f| {
                        f.push(&binary_path);
                        let appended_path = f.as_path();
                        if appended_path.exists() && appended_path.is_file() {
                            Some(appended_path.to_path_buf())
                        } else {
                            None
                        }
                    }).next() {
                        Ok(Job {
                            status: Status::NotStarted,
                            output: None,
                            configuration: Configuration::Command(binary_appended_path, str_arguments, fd_options),
                            background: background
                        })
                    } else {
                        Err(Error::CommandNotFound(binary_path))
                    }
                } else {
                    Err(Error::CorruptPath)
                }
            },
            &Expr::Pipeline(ref first, ref second) => {
                let first_result = Job::from_expr(&first, is_builtin);
                let second_result = Job::from_expr(&second, is_builtin);
                if let Ok(f) = first_result {
                    if let Ok(s) = second_result {
                        Ok(Job {
                            status: Status::NotStarted,
                            output: None,
                            configuration: Configuration::Pipeline(Rc::new(f), Rc::new(s)),
                            background: false
                        })
                    } else {
                        Err(Error::RightPipe(Rc::new(second_result.unwrap_err())))
                    }
                } else {
                    Err(Error::LeftPipe(Rc::new(first_result.unwrap_err())))
                }
            }
        }
        /*
        if let Ok(binary_cstring) = CString::new(binary) {
            let mut args_cstring: Vec<CString> = Vec::new();
            args_cstring.push(binary_cstring.clone());
            for arg in args {
                if let Ok(arg_cstring) = CString::new(*arg) {
                    args_cstring.push(arg_cstring);
                } else {
                    return None;
                }
            }
            if let Ok(fork_result) = nix::unistd::fork() {
                match fork_result {
                    nix::unistd::ForkResult::Parent{child} => {
                        Some(Job{ pid: child })
                    }
                    nix::unistd::ForkResult::Child => {
                        nix::unistd::execvp(&binary_cstring, &args_cstring);
                        process::exit(-1);
                    }
                }
            } else {
                None
            }
        } else {
            None
        }
        */
    }

    pub fn wait(&self) {
        //nix::sys::wait::waitpid(self.pid, None);
    }

    pub fn run(&mut self) -> Result<Status, Error> {
        println!("{:?}", self);
        self.run_with_fd(None, None)
    }

    pub fn run_with_output(&mut self) -> Result<String, Error> {
        Ok(String::from(""))
    }

    fn run_with_fd(&mut self, input_fd: Option<u32>, output_fd: Option<u32>) -> Result<Status, Error> {
        /*
        match self.configuration {
            Configuration::Command(binary, arguments, fd_options) => {

            }
    }*/
        Ok(Status::Running(0))
    }
}

fn join_components<'a>(components: &'a [StringLiteralComponent<'a>]) -> String {
    let strs: Vec<String> = components.into_iter().map(|s| {
        match s {
            &StringLiteralComponent::Literal(s) => String::from(s),
            &StringLiteralComponent::EnvVar(v) => env::var(v).unwrap_or(String::from("")),
        }
    }).collect();
    strs.join("")
}
