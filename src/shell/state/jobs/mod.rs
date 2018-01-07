use std::process;
use nix;
use std::ffi::CString;
use super::super::syntax::ast::{Argument, Expr};
use super::super::syntax::tokens::StringLiteralComponent;
use std::collections::HashMap;
use std::path::PathBuf;
use std::env;
use std::os::unix::io::{FromRawFd, RawFd};
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::io::{stdin, stdout};
use std::os::unix::io::AsRawFd;
use std::ops::Deref;
use std::sync::RwLock;
extern crate glob;
use self::glob::glob;

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
    Pipeline(Box<Job>, Box<Job>),
}

#[derive(Debug, Clone, Copy)]
pub enum Status {
    NotStarted,
    Started(
        nix::libc::pid_t, /* pid */
        nix::libc::pid_t, /* pgid */
        nix::sys::wait::WaitStatus,
    ),
}

#[derive(Debug)]
pub enum Error {
    Fork,
    StringEncoding,
    Subshell(Box<Error>),
    SubshellExecution,
    CommandNotFound(PathBuf),
    CorruptPath,
    LeftPipe(Box<Error>),
    RightPipe(Box<Error>),
    Pipe,
    Wait,
}

#[derive(Debug)]
pub struct Job {
    pub status: RwLock<Status>,
    pub output: Option<String>,
    configuration: Configuration,
    pub background: bool,
}

pub trait BuiltinHandler {
    fn handle_builtin(&mut self, name: &str, args: &[String]) -> i8;
    fn is_builtin(&mut self, name: &str) -> bool;
}

impl Drop for Job {
    fn drop(&mut self) {
        match self.configuration {
            Configuration::Command(_, _, _) => match self.get_status() {
                Status::Started(pid, _, _) => {
                    match nix::sys::signal::kill(pid, nix::sys::signal::Signal::SIGKILL) {
                        Ok(_) => {
                            eprintln!("warning: killed {} with SIGKILL", pid);
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Job {
    pub fn from_expr<B: BuiltinHandler>(
        expr: &Expr,
        builtin_handler: &mut B,
    ) -> Result<Job, Error> {
        match expr {
            &Expr::Command(binary, ref arguments) => {
                let mut fd_options = HashMap::<RawFd, FdOption>::new();
                let mut background = false;
                let mut str_arguments = Vec::<String>::new();
                for arg in arguments {
                    match arg {
                        &Argument::Redirect(fd, path) => {
                            fd_options.insert(
                                fd,
                                FdOption::Overwrite(PathBuf::from(join_components(path))),
                            );
                        }
                        &Argument::RedirectFD(fd, target) => {
                            fd_options.insert(fd, FdOption::Fd(target));
                        }
                        &Argument::Append(fd, path) => {
                            fd_options
                                .insert(fd, FdOption::Append(PathBuf::from(join_components(path))));
                        }
                        &Argument::Input(fd, path) => {
                            fd_options
                                .insert(fd, FdOption::Input(PathBuf::from(join_components(path))));
                        }
                        &Argument::Background => {
                            background = true;
                        }
                        &Argument::Subshell(ref subexpr) => match Job::from_expr(
                            &subexpr,
                            builtin_handler,
                        ) {
                            Ok(mut subjob) => match subjob.run_with_output(builtin_handler) {
                                Ok(output) => {
                                    let parts: Vec<&str> = output.split_whitespace().collect();
                                    str_arguments.push(parts.join(" "));
                                }
                                Err(error) => {
                                    return Err(Error::Subshell(Box::new(error)));
                                }
                            },
                            Err(error) => {
                                return Err(Error::Subshell(Box::new(error)));
                            }
                        },
                        &Argument::Literal(s) => {
                            let joined = join_components(s);
                            if let Ok(it) = glob(&joined) {
                                let mut glob_components: Vec<
                                    String,
                                > = Vec::new();
                                let mut glob_valid = true;
                                for entry in it {
                                    match entry {
                                        Ok(path) => {
                                            if let Some(path_str) = path.to_str() {
                                                glob_components.push(String::from(path_str));
                                            } else {
                                                glob_valid = false;
                                                break;
                                            }
                                        }
                                        Err(_) => {
                                            glob_valid = false;
                                            break;
                                        }
                                    }
                                }
                                if glob_valid && glob_components.len() > 0 {
                                    str_arguments.append(&mut glob_components);
                                } else {
                                    str_arguments.push(joined);
                                }
                            } else {
                                str_arguments.push(joined);
                            }
                        }
                    };
                }
                let binary_str = join_components(binary);
                if builtin_handler.is_builtin(&binary_str) {
                    Ok(Job {
                        status: RwLock::new(Status::NotStarted),
                        output: None,
                        configuration: Configuration::Builtin(
                            binary_str,
                            str_arguments,
                            fd_options,
                        ),
                        background: background,
                    })
                } else if let Some(path_os_str) = env::var_os("PATH") {
                    let binary_path = PathBuf::from(&binary_str);
                    if let Ok(resolved) = binary_path.canonicalize() {
                        if resolved.is_file() {
                            Ok(Job {
                                status: RwLock::new(Status::NotStarted),
                                output: None,
                                configuration: Configuration::Command(
                                    resolved,
                                    str_arguments,
                                    fd_options,
                                ),
                                background: background,
                            })
                        } else {
                            Err(Error::CommandNotFound(binary_path))
                        }
                    } else {
                        if let Some(binary_appended_path) = env::split_paths(&path_os_str)
                            .filter_map(|mut f| {
                                f.push(&binary_path);
                                let appended_path = f.as_path();
                                if appended_path.exists() && appended_path.is_file() {
                                    Some(appended_path.to_path_buf())
                                } else {
                                    None
                                }
                            })
                            .next()
                        {
                            Ok(Job {
                                status: RwLock::new(Status::NotStarted),
                                output: None,
                                configuration: Configuration::Command(
                                    binary_appended_path,
                                    str_arguments,
                                    fd_options,
                                ),
                                background: background,
                            })
                        } else {
                            Err(Error::CommandNotFound(binary_path))
                        }
                    }
                } else {
                    Err(Error::CorruptPath)
                }
            }
            &Expr::Pipeline(ref first, ref second) => {
                let first_result = Job::from_expr(&first, builtin_handler);
                let second_result = Job::from_expr(&second, builtin_handler);
                if let Ok(f) = first_result {
                    if let Ok(s) = second_result {
                        Ok(Job {
                            status: RwLock::new(Status::NotStarted),
                            output: None,
                            configuration: Configuration::Pipeline(Box::new(f), Box::new(s)),
                            background: false,
                        })
                    } else {
                        Err(Error::RightPipe(Box::new(second_result.unwrap_err())))
                    }
                } else {
                    Err(Error::LeftPipe(Box::new(first_result.unwrap_err())))
                }
            }
        }
    }

    pub fn cont(&mut self, background: bool) -> nix::Result<()> {
        let new_status = {
            match self.get_status() {
                Status::Started(pid, pgid, nix::sys::wait::WaitStatus::Stopped(_, _)) => {
                    if !background {
                        assert!(self.in_foreground());
                    }
                    match self.configuration {
                        Configuration::Builtin(_, _, _) => {
                            panic!("builtin should never be stopped");
                        }
                        Configuration::Pipeline(ref mut first, ref mut second) => {
                            let first_result = first.cont(background);
                            let second_result = second.cont(background);
                            let result = first_result.or(second_result);
                            match result {
                                Ok(_) => Ok(Status::Started(
                                    pid,
                                    pgid,
                                    nix::sys::wait::WaitStatus::StillAlive,
                                )),
                                Err(e) => Err(e),
                            }
                        }
                        Configuration::Command(_, _, _) => {
                            let result = nix::sys::signal::kill(-pgid, nix::sys::signal::SIGCONT);
                            match result {
                                Ok(_) => Ok(Status::Started(
                                    pid,
                                    pgid,
                                    nix::sys::wait::WaitStatus::StillAlive,
                                )),
                                Err(e) => Err(e),
                            }
                        }
                    }
                }
                s => Ok(s),
            }
        };
        match new_status {
            Ok(s) => {
                self.set_status(s);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    fn set_term_group(&self, pgid: nix::libc::pid_t) {
        let block_sigaction = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::SigIgn,
            nix::sys::signal::SaFlags::empty(),
            nix::sys::signal::SigSet::empty(),
        );
        let default_sigaction = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::SigDfl,
            nix::sys::signal::SaFlags::empty(),
            nix::sys::signal::SigSet::empty(),
        );
        unsafe {
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGTTOU, &block_sigaction)
                .expect("failed to block SIGTTOU");
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGTTIN, &block_sigaction)
                .expect("failed to block SIGTTIN");
            nix::unistd::tcsetpgrp(stdin().as_raw_fd(), pgid)
                .expect("failed to reset terminal group");
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGTTOU, &default_sigaction)
                .expect("failed to restore SIGTTOU");
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGTTIN, &default_sigaction)
                .expect("failed to restore SIGTTIN");
        }
    }

    pub fn set_foreground(&self) {
        match self.get_status() {
            Status::NotStarted => {
                panic!("cannot set foreground if job has not started");
            }
            Status::Started(_, pgid, _) => {
                self.set_term_group(pgid);
            }
        }
    }

    pub fn in_foreground(&self) -> bool {
        match self.get_status() {
            Status::NotStarted => false,
            Status::Started(_, pgid, _) => {
                let current_pgid =
                    nix::unistd::tcgetpgrp(stdin().as_raw_fd()).expect("failed to tcgetpgrp stdin");
                pgid == current_pgid
            }
        }
    }

    pub fn wait(
        &mut self,
        flags: Option<nix::sys::wait::WaitPidFlag>,
    ) -> nix::Result<nix::sys::wait::WaitStatus> {
        let result = self.wait_without_restore(flags);
        self.set_term_group(nix::unistd::getpgid(None).unwrap());
        result
    }

    fn wait_without_restore(
        &mut self,
        flags: Option<nix::sys::wait::WaitPidFlag>,
    ) -> nix::Result<nix::sys::wait::WaitStatus> {
        let result_status = {
            match self.configuration {
                Configuration::Builtin(_, _, _) => match self.get_status() {
                    Status::Started(_, _, s) => Ok((s, self.get_status())),
                    Status::NotStarted => panic!("builtin should not be running"),
                },
                Configuration::Command(_, _, _) => match self.get_status() {
                    Status::Started(pid, pgid, status) => match status {
                        nix::sys::wait::WaitStatus::StillAlive
                        | nix::sys::wait::WaitStatus::Continued(_) => {
                            let wait_result = nix::sys::wait::waitpid(pid, flags);
                            match wait_result {
                                Ok(result) => Ok((result, Status::Started(pid, pgid, result))),
                                Err(e) => Err(e),
                            }
                        }
                        _ => Ok((status, Status::Started(pid, pgid, status))),
                    },
                    Status::NotStarted => Err(nix::Error::from_errno(nix::Errno::ESRCH)),
                },
                Configuration::Pipeline(ref mut first, ref mut second) => {
                    match first.wait_without_restore(flags) {
                        Ok(_) => match second.wait_without_restore(flags) {
                            Ok(r) => Ok((r, second.get_status())),
                            Err(e) => Err(e),
                        },
                        Err(e) => Err(e),
                    }
                }
            }
        };
        match result_status {
            Ok((actual_result, new_status)) => {
                self.set_status(new_status);
                Ok(actual_result)
            }
            Err(e) => Err(e),
        }
    }

    fn set_status(&mut self, status: Status) {
        *self.status.write().unwrap() = status;
    }

    pub fn get_status(&self) -> Status {
        self.status.read().unwrap().deref().clone()
    }

    pub fn run<B: BuiltinHandler>(&mut self, handler: &mut B) -> Result<Status, Error> {
        //println!("{:?}", self);
        self.run_with_fd(None, None, handler, &vec![], None)
    }

    pub fn run_with_output<B: BuiltinHandler>(&mut self, handler: &mut B) -> Result<String, Error> {
        match nix::unistd::pipe() {
            Ok((output, input)) => {
                match self.run_with_fd(None, Some(input), handler, &vec![output], None) {
                    Ok(_) => {
                        if let Ok(nix::sys::wait::WaitStatus::Exited(_, _)) = self.wait(None) {
                            if let Err(_) = nix::unistd::close(input) {
                                Err(Error::Pipe)
                            } else {
                                unsafe {
                                    // note: the fd is not a file, but Rust's api seems to work anyway
                                    let mut output_file = File::from_raw_fd(output);
                                    let mut contents = String::new();
                                    if let Err(_) = output_file.read_to_string(&mut contents) {
                                        Err(Error::Pipe)
                                    } else {
                                        Ok(contents)
                                    }
                                }
                            }
                        } else {
                            Err(Error::SubshellExecution)
                        }
                    }
                    Err(e) => Err(e),
                }
            }
            Err(_) => Err(Error::Pipe),
        }
    }

    fn run_with_fd<B: BuiltinHandler>(
        &mut self,
        input_fd: Option<RawFd>,
        output_fd: Option<RawFd>,
        handler: &mut B,
        post_fork_close: &[RawFd],
        pgid: Option<nix::libc::pid_t>,
    ) -> Result<Status, Error> {
        match self.get_status() {
            Status::NotStarted => {}
            _ => {
                panic!("cannot re-run already running job");
            }
        };
        use nix::fcntl::*;
        use nix::sys::stat::*;
        fn apply_fd_changes(
            input_fd: Option<RawFd>,
            output_fd: Option<RawFd>,
            options: &HashMap<RawFd, FdOption>,
        ) -> (bool, Vec<(RawFd, RawFd, Option<RawFd>)>) {
            let mut log: Vec<(RawFd, RawFd, Option<RawFd>)> = Vec::new();
            if let Some(input) = input_fd {
                match nix::unistd::dup(stdin().as_raw_fd()) {
                    Ok(saved) => log.push((stdin().as_raw_fd(), saved, None)),
                    Err(_) => return (false, log),
                }
                if let Err(_) = nix::unistd::dup2(input, stdin().as_raw_fd()) {
                    return (false, log);
                }
            }
            if let Some(output) = output_fd {
                match nix::unistd::dup(stdout().as_raw_fd()) {
                    Ok(saved) => log.push((stdout().as_raw_fd(), saved, None)),
                    Err(_) => return (false, log),
                }
                if let Err(_) = nix::unistd::dup2(output, stdout().as_raw_fd()) {
                    return (false, log);
                }
            }
            for (src, opt) in options {
                match nix::unistd::dup(*src) {
                    Ok(saved) => {
                        match opt {
                            &FdOption::Fd(dest) => {
                                log.push((*src, saved, None));
                                if let Err(_) = nix::unistd::dup2(dest, *src) {
                                    return (false, log);
                                }
                            }
                            &FdOption::Append(ref path) => match nix::fcntl::open(
                                path,
                                O_WRONLY | O_CREAT | O_APPEND,
                                S_IWUSR | S_IRUSR | S_IRGRP | S_IROTH,
                            ) {
                                Ok(newfd) => {
                                    log.push((*src, saved, Some(newfd)));
                                    if let Err(_) = nix::unistd::dup2(newfd, *src) {
                                        return (false, log);
                                    }
                                }
                                Err(_) => {
                                    log.push((*src, saved, None));
                                    return (false, log);
                                }
                            },
                            &FdOption::Overwrite(ref path) => {
                                if path.exists() && path.is_file() {
                                    match fs::remove_file(path) {
                                        Ok(_) => {}
                                        Err(_) => return (false, log),
                                    }
                                }
                                match nix::fcntl::open(
                                    path,
                                    O_WRONLY | O_CREAT,
                                    S_IWUSR | S_IRUSR | S_IRGRP | S_IROTH,
                                ) {
                                    Ok(newfd) => {
                                        log.push((*src, saved, Some(newfd)));
                                        if let Err(_) = nix::unistd::dup2(newfd, *src) {
                                            return (false, log);
                                        }
                                    }
                                    Err(_) => {
                                        log.push((*src, saved, None));
                                        return (false, log);
                                    }
                                }
                            }
                            &FdOption::Input(ref path) => {
                                match nix::fcntl::open(path, O_RDONLY, S_IWUSR /* ignored */) {
                                    Ok(newfd) => {
                                        log.push((*src, saved, Some(newfd)));
                                        if let Err(_) = nix::unistd::dup2(newfd, *src) {
                                            return (false, log);
                                        }
                                    }
                                    Err(_) => {
                                        log.push((*src, saved, None));
                                        return (false, log);
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => return (false, log),
                }
            }
            (true, log)
        }

        fn reverse_fd_changes(log: &Vec<(RawFd, RawFd, Option<RawFd>)>) {
            log.iter().rev().for_each(|&(src, saved, newfd)| {
                if let Err(_) = nix::unistd::dup2(saved, src) {
                    panic!("failed to revert file descriptors");
                }

                if let Err(_) = nix::unistd::close(saved) {
                    panic!("failed to revert file descriptors");
                }

                if let Some(n) = newfd {
                    if let Err(_) = nix::unistd::close(n) {
                        panic!("failed to revert file descriptors");
                    }
                }
            });
        }
        let result = {
            match self.configuration {
                Configuration::Builtin(ref name, ref args, ref options) => {
                    let (success, log) = apply_fd_changes(input_fd, output_fd, options);
                    let mut result: i8 = -1;
                    if success {
                        result = handler.handle_builtin(&name, &args);
                    }
                    reverse_fd_changes(&log);
                    Ok(Status::Started(
                        nix::unistd::getpid(),
                        nix::unistd::getpgid(None).unwrap(), /* should always succeed */
                        nix::sys::wait::WaitStatus::Exited(nix::unistd::getpid(), result),
                    ))
                }
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
                                    nix::unistd::ForkResult::Parent { child } => {
                                        let child_pgid = pgid.unwrap_or(child);
                                        nix::unistd::setpgid(child, child_pgid)
                                            .expect("failed to set process group for child");
                                        if !self.background {
                                            if let Ok(existing_group) =
                                                nix::unistd::tcgetpgrp(stdin().as_raw_fd())
                                            {
                                                if existing_group != child_pgid {
                                                    nix::unistd::tcsetpgrp(stdin().as_raw_fd(), child_pgid)
                                                        .expect("failed to tcsetpgrp stdin");
                                                }
                                            }
                                        }
                                        Ok(Status::Started(
                                            child,
                                            child_pgid,
                                            nix::sys::wait::WaitStatus::StillAlive,
                                        ))
                                    }
                                    nix::unistd::ForkResult::Child => {
                                        let (mut success, _) =
                                            apply_fd_changes(input_fd, output_fd, options);
                                        post_fork_close.into_iter().for_each(|fd| {
                                            if nix::unistd::close(*fd).is_err() {
                                                success = false;
                                            }
                                        });
                                        if success {
                                            if let Some(child_pgid) = pgid {
                                                nix::unistd::setpgid(0 /* self */, child_pgid)
                                                    .expect(
                                                        "failed to setpgid to child pgid in child",
                                                    );
                                            } else {
                                                let child_pid = nix::unistd::getpid();
                                                nix::unistd::setpgid(0 /* self */, child_pid)
                                                    .expect(
                                                        "failed to setpgid to child pid in child",
                                                    );
                                                //nix::unistd::setsid().expect("failed to create new session/process group in child");
                                            }
                                            let default_sigaction =
                                                nix::sys::signal::SigAction::new(
                                                    nix::sys::signal::SigHandler::SigDfl,
                                                    nix::sys::signal::SaFlags::empty(),
                                                    nix::sys::signal::SigSet::empty(),
                                                );
                                            unsafe {
                                                nix::sys::signal::sigaction(
                                                    nix::sys::signal::Signal::SIGINT,
                                                    &default_sigaction,
                                                ).expect("failed to set SIGINT");
                                                nix::sys::signal::sigaction(
                                                    nix::sys::signal::Signal::SIGTSTP,
                                                    &default_sigaction,
                                                ).expect("failed to set SIGSTP");
                                                nix::sys::signal::sigaction(
                                                    nix::sys::signal::Signal::SIGQUIT,
                                                    &default_sigaction,
                                                ).expect("failed to set SIGQUIT");
                                                nix::sys::signal::sigaction(
                                                    nix::sys::signal::Signal::SIGPIPE,
                                                    &default_sigaction,
                                                ).expect("failed to set SIGPIPE");
                                            }
                                            match nix::unistd::execvp(
                                                &binary_cstring,
                                                &args_cstring,
                                            ) {
                                                _ => {}
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
                Configuration::Pipeline(ref mut first, ref mut second) => match nix::unistd::pipe()
                {
                    Ok((output, input)) => {
                        let result: Result<Status, Error>;
                        let mut first_cleanup = Vec::from(post_fork_close);
                        first_cleanup.push(output);
                        match first.run_with_fd(
                            input_fd,
                            Some(input),
                            handler,
                            &first_cleanup,
                            pgid,
                        ) {
                            Ok(status) => match status {
                                Status::NotStarted => {
                                    panic!("pipeline has not started");
                                }
                                Status::Started(_, first_pgid, _) => {
                                    let mut second_cleanup = Vec::from(post_fork_close);
                                    second_cleanup.push(input);
                                    match second.run_with_fd(
                                        Some(output),
                                        output_fd,
                                        handler,
                                        &second_cleanup,
                                        Some(first_pgid),
                                    ) {
                                        Ok(s) => {
                                            result = Ok(s);
                                        }
                                        Err(e) => {
                                            result = Err(Error::RightPipe(Box::new(e)));
                                        }
                                    };
                                }
                            },
                            Err(e) => {
                                result = Err(Error::LeftPipe(Box::new(e)));
                            }
                        };
                        if nix::unistd::close(input).is_ok() && nix::unistd::close(output).is_ok() {
                            result
                        } else {
                            Err(Error::Pipe)
                        }
                    }
                    Err(_) => Err(Error::Pipe),
                },
            }
        };
        if let Ok(status) = result {
            self.set_status(status);
        }
        result
    }
}

fn join_components<'a>(components: &'a [StringLiteralComponent<'a>]) -> String {
    let strs: Vec<String> = components
        .into_iter()
        .map(|s| match s {
            &StringLiteralComponent::Literal(s) => String::from(s),
            &StringLiteralComponent::EnvVar(v) => env::var(v).unwrap_or(String::from("")),
            _ => String::from(""),
        })
        .collect();
    strs.join("")
}
