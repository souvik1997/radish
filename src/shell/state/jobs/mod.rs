use std::process;
use nix;
use std::ffi::CString;
use super::super::syntax::ast::{Expr, Argument};
use super::super::syntax::tokens::StringLiteralComponent;
use super::ShellState;
use std::collections::HashMap;
use std::rc::Rc;
use std::path::PathBuf;
use std::env;
use std::fmt;
use std::cell::Cell;
use std::os::unix::io::RawFd;

#[derive(Debug)]
pub enum FdOption {
    Append(PathBuf),
    Overwrite(PathBuf),
    Input(PathBuf),
    Fd(RawFd),
}

#[derive(Debug)]
pub enum Configuration {
    Command(PathBuf, Vec<String>, HashMap<RawFd, FdOption>),
    Builtin(String, Vec<String>, HashMap<RawFd, FdOption>),
    Pipeline(Rc<Job>, Rc<Job>)
}

#[derive(Debug, Clone, Copy)]
pub enum Status {
    NotStarted,
    Running(nix::libc::pid_t),
    Done(i8)
}

#[derive(Debug)]
pub enum Error {
    Fork,
    StringEncoding,
    Subshell(Rc<Error>),
    CommandNotFound(PathBuf),
    CorruptPath,
    LeftPipe(Rc<Error>),
    RightPipe(Rc<Error>),
}

#[derive(Debug)]
pub struct Job {
    status: Cell<Status>,
    pub output: Option<String>,
    configuration: Configuration,
    pub background: bool
}

pub trait BuiltinHandler {
    fn handle_builtin(&mut self, name: &str, args: &[String]) -> i8;
    fn is_builtin(&mut self, name: &str) -> bool;
}

impl Job {
    pub fn from_expr<B: BuiltinHandler>(expr: &Expr, builtin_handler: &mut B) -> Result<Job, Error> {
        match expr {
            &Expr::Command(binary, ref arguments) => {
                let mut fd_options = HashMap::<RawFd, FdOption>::new();
                let mut background = false;
                let mut str_arguments = Vec::<String>::new();
                for arg in arguments {
                    match arg {
                        &Argument::Redirect(fd, path) => {
                            fd_options.insert(fd, FdOption::Overwrite(PathBuf::from(join_components(path))));
                        },
                        &Argument::RedirectFD(fd, target) => { fd_options.insert(fd, FdOption::Fd(target)); },
                        &Argument::Append(fd, path) => { fd_options.insert(fd, FdOption::Append(PathBuf::from(join_components(path)))); },
                        &Argument::Input(fd, path) => { fd_options.insert(fd, FdOption::Input(PathBuf::from(join_components(path)))); },
                        &Argument::Background => { background = true; },
                        &Argument::Subshell(ref subexpr) => {
                            match Job::from_expr(&subexpr, builtin_handler) {
                                Ok(mut subjob) => {
                                    match subjob.run_with_output(builtin_handler) {
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
                if builtin_handler.is_builtin(&binary_str) {
                    Ok(Job {
                        status: Cell::new(Status::NotStarted),
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
                            status: Cell::new(Status::NotStarted),
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
                let first_result = Job::from_expr(&first, builtin_handler);
                let second_result = Job::from_expr(&second, builtin_handler);
                if let Ok(f) = first_result {
                    if let Ok(s) = second_result {
                        Ok(Job {
                            status: Cell::new(Status::NotStarted),
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

    pub fn wait_until_complete(&self) -> nix::Result<nix::sys::wait::WaitStatus> {
        self.wait(None)
    }

    fn wait(&self, flags: Option<nix::sys::wait::WaitPidFlag>) -> nix::Result<nix::sys::wait::WaitStatus> {
        match self.configuration {
            Configuration::Builtin(_, _, _) => {
                if let Status::Done(status) = self.status.get() {
                    Ok(nix::sys::wait::WaitStatus::Exited(nix::unistd::getpid(), status))
                } else {
                    panic!("builtin should not be running")
                }
            },
            Configuration::Command(_, _, _) => {
                match self.status.get() {
                    Status::Done(status) => {
                        Ok(nix::sys::wait::WaitStatus::Exited(nix::unistd::getpid(), status))
                    },
                    Status::Running(pid) => {
                        let result = nix::sys::wait::waitpid(pid, flags);
                        if let Ok(nix::sys::wait::WaitStatus::Exited(_, status)) = result {
                            self.status.set(Status::Done(status));
                        }
                        result
                    },
                    Status::NotStarted => {
                        Err(nix::Error::from_errno(nix::Errno::EINVAL))
                    }
                }
            },
            Configuration::Pipeline(_, ref second) => {
                second.wait(flags)
            }
        }
        //nix::sys::wait::waitpid(self.pid, None);
    }

    pub fn run<B: BuiltinHandler>(&mut self, handler: &mut B) -> Result<Status, Error> {
        //println!("{:?}", self);
        self.run_with_fd(None, None, handler)
    }

    pub fn run_with_output<B: BuiltinHandler>(&mut self, handler: &mut B) -> Result<String, Error> {
        Ok(String::from(""))
    }

    fn run_with_fd<B: BuiltinHandler>(&mut self, input_fd: Option<RawFd>, output_fd: Option<RawFd>, handler: &mut B) -> Result<Status, Error> {
        use nix::fcntl::*;
        use nix::sys::stat::*;
        fn apply_fd_changes(input_fd: Option<RawFd>, output_fd: Option<RawFd>, options: &HashMap<RawFd, FdOption>) -> (bool, Vec<(RawFd, RawFd)>) {
            let mut log: Vec<(RawFd, RawFd)> = Vec::new();
            if let Some(input) = input_fd {
                match nix::unistd::dup(0) {
                    Ok(saved) => log.push((0, saved)),
                    Err(_) => return (false, log)
                }
                if let Err(_) = nix::unistd::dup2(input, 0) {
                    return (false, log);
                }
            }
            if let Some(output) = output_fd {
                match nix::unistd::dup(1) {
                    Ok(saved) => log.push((1, saved)),
                    Err(_) => return (false, log)
                }
                if let Err(_) = nix::unistd::dup2(output, 0) {
                    return (false, log);
                }
            }
            for (src, opt) in options {
                match nix::unistd::dup(*src) {
                    Ok(saved) => log.push((*src, saved)),
                    Err(_) => return (false, log)
                }
                match opt {
                    &FdOption::Fd(dest) => {
                        if let Err(_) = nix::unistd::dup2(dest, *src) {
                            return (false, log)
                        }
                    },
                    &FdOption::Append(ref path) => {
                        match nix::fcntl::open(path, O_WRONLY | O_CREAT | O_APPEND, S_IWUSR | S_IRUSR | S_IRGRP | S_IROTH) {
                            Ok(newfd) => {
                                if let Err(_) = nix::unistd::dup2(newfd, *src) {
                                    return (false, log)
                                }
                            },
                            Err(_) => return (false, log)
                        }
                    },
                    &FdOption::Overwrite(ref path) => {
                        match nix::fcntl::open(path, O_WRONLY | O_CREAT, S_IWUSR | S_IRUSR | S_IRGRP | S_IROTH) {
                            Ok(newfd) => {
                                if let Err(_) = nix::unistd::dup2(newfd, *src) {
                                    return (false, log)
                                }
                            },
                            Err(_) => return (false, log)
                        }
                    },
                    &FdOption::Input(ref path) => {
                        match nix::fcntl::open(path, O_RDONLY, S_IWUSR /* ignored */) {
                            Ok(newfd) => {
                                if let Err(_) = nix::unistd::dup2(newfd, *src) {
                                    return (false, log)
                                }
                            },
                            Err(_) => return (false, log)
                        }
                    }

                }
            }
            (true, log)
        }

        fn reverse_fd_changes(log: &Vec<(RawFd, RawFd)>) {
            log.iter().rev().for_each(|&(src, saved)| {
                if let Err(_) = nix::unistd::dup2(saved, src) {
                    panic!("failed to revert file descriptors");
                }
            });
        }
        match self.configuration {
            Configuration::Builtin(ref name, ref args, ref options) => {
                let (success, log) = apply_fd_changes(input_fd, output_fd, options);
                let mut result: i8 = -1;
                if success {
                    result = handler.handle_builtin(&name, &args);
                }
                reverse_fd_changes(&log);
                self.status.set(Status::Done(result));
                Ok(self.status.get())
            },
            Configuration::Command(ref path, ref args, ref options) => {
                if let Some(path_str) = path.to_str() {
                    if let Ok(binary_cstring) = CString::new(path_str) {
                        let mut args_cstring: Vec<CString> = Vec::new();
                        args_cstring.push(binary_cstring.clone());
                        for arg in args {
                            if let Ok(arg_cstring) = CString::new(arg.clone()) {
                                args_cstring.push(arg_cstring);
                            } else {
                                return Err(Error::StringEncoding);
                            }
                        }
                        if let Ok(fork_result) = nix::unistd::fork() {
                            match fork_result {
                                nix::unistd::ForkResult::Parent{child} => {
                                    self.status.set(Status::Running(child));
                                    Ok(self.status.get())
                                },
                                nix::unistd::ForkResult::Child => {
                                    let (success, _) = apply_fd_changes(input_fd, output_fd, options);
                                    if success {
                                        match nix::unistd::execvp(&binary_cstring, &args_cstring) {
                                            _ => { process::exit(-1) }
                                        }
                                    }
                                    process::exit(-1);
                                }
                            }
                        } else {
                            Err(Error::Fork)
                        }
                    } else {
                        Err(Error::StringEncoding)
                    }
                } else {
                    Err(Error::StringEncoding)
                }
            }
            _ => { Err(Error::Fork) }
        }
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
