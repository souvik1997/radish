use std::process;
use nix;
use std::ffi::CString;
use super::syntax::ast::{Expr, Argument};
use super::syntax::tokens::StringLiteralComponent;
use std::collections::HashMap;
use std::rc::Rc;
use std::path::PathBuf;
use std::env;

#[derive(Debug)]
enum FdOptions {
    Append(PathBuf),
    Overwrite(PathBuf),
    Input(PathBuf),
    Fd(u32),
}

#[derive(Debug)]
enum Configuration {
    Command(String, Vec<String>, HashMap<u32, FdOptions>),
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
    ForkError,
    SubshellError,
}

#[derive(Debug)]
pub struct Job {
    status: Status,
    output: Option<String>,
    configuration: Configuration,
    background: bool
}

impl Job {
    pub fn new(expr: &Expr) -> Result<Job, Error> {
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
                            match Job::new(&subexpr) {
                                Ok(mut subjob) => {
                                    match subjob.run_with_output() {
                                        Ok(output) => {
                                            str_arguments.push(output);
                                        }
                                        Err(error) => {
                                            return Err(error);
                                        }
                                    }
                                },
                                Err(error) => {
                                    return Err(error);
                                }
                            }
                        },
                        &Argument::Literal(s) => {
                            str_arguments.push(String::from(join_components(s)));
                        }
                    };
                }
                Ok(Job {
                    status: Status::NotStarted,
                    output: None,
                    configuration: Configuration::Command(String::from(join_components(binary)), str_arguments, fd_options),
                    background: background
                })
            },
            &Expr::Pipeline(ref first, ref second) => {
                let first_result = Job::new(&first);
                let second_result = Job::new(&second);
                if let Ok(f) = first_result {
                    if let Ok(s) = second_result {
                        Ok(Job {
                            status: Status::NotStarted,
                            output: None,
                            configuration: Configuration::Pipeline(Rc::new(f), Rc::new(s)),
                            background: false
                        })
                    } else {
                        second_result
                    }
                } else {
                    first_result
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
