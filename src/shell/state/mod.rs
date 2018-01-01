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
use std::sync::RwLock;
use nix;


pub struct ShellState {
    background_jobs: RwLock<Vec<jobs::Job>>,
    foreground_jobs: RwLock<Vec<jobs::Job>>,
    stopped_jobs: RwLock<Vec<jobs::Job>>,
    current_job_pid: RwLock<Cell<Option<nix::libc::pid_t>>>,
    pub ketos_interp: Interpreter,
}

impl ShellState {
    pub fn new() -> Self {
        ShellState {
            background_jobs: RwLock::new(Vec::<jobs::Job>::new()),
            foreground_jobs: RwLock::new(Vec::<jobs::Job>::new()),
            stopped_jobs: RwLock::new(Vec::<jobs::Job>::new()),
            current_job_pid: RwLock::new(Cell::new(None)),
            ketos_interp: Interpreter::new()
        }
    }

    pub fn enqueue_job(&mut self, expr: &Expr) -> Result<(), jobs::Error> {
        match jobs::Job::from_expr(&expr, self) {
            Ok(mut job) => {
                if job.background {
                    match job.run(self) {
                        Ok(_) => {
                            let mut background_jobs = self.background_jobs.write().unwrap();
                            background_jobs.push(job);
                            Ok(())
                        },
                        Err(e) => Err(e)
                    }
                } else {
                    let mut foreground_jobs = self.foreground_jobs.write().unwrap();
                    foreground_jobs.push(job);
                    Ok(())
                }
            },
            Err(e) => Err(e)
        }
    }

    pub fn run_foreground_jobs(&mut self) -> Result<(), jobs::Error> {
        fn get_next_job(queue: &RwLock<Vec<jobs::Job>>) -> Option<jobs::Job> {
            let mut foreground_jobs = queue.write().unwrap();
            if foreground_jobs.len() > 0 {
                let job = foreground_jobs.remove(0);
                Some(job)
            } else {
                None
            }
        }
        while let Some(mut job) = get_next_job(&self.foreground_jobs) {
            self.current_job_pid.write().unwrap().set(None);
            loop {
                match job.status.get() {
                    jobs::Status::NotStarted => {
                        match job.run(self) {
                            Ok(_) => { },
                            Err(e) => {
                                return Err(e);
                            }
                        }
                    }
                    jobs::Status::Started(pid, _, status) => {
                        self.current_job_pid.write().unwrap().set(Some(pid));
                        match status {
                            nix::sys::wait::WaitStatus::StillAlive | nix::sys::wait::WaitStatus::Continued(_) => {
                                match job.wait(Some(nix::sys::wait::WUNTRACED)) {
                                    Ok(nix::sys::wait::WaitStatus::Stopped(_,_)) => {
                                        self.stopped_jobs.write().unwrap().push(job);
                                        break;
                                    },
                                    Ok(nix::sys::wait::WaitStatus::Exited(_,_)) | Ok(nix::sys::wait::WaitStatus::Signaled(_,_,_))=> {
                                        break;
                                    },
                                    Ok(nix::sys::wait::WaitStatus::StillAlive) => {
                                        panic!("job should not be still running since waitpid was not called with WNOHANG");
                                    },
                                    Ok(nix::sys::wait::WaitStatus::Continued(_)) => {
                                        panic!("job should not be continued since waitpid was not called with WCONTINUED");
                                    }
                                    #[cfg(not(target_os="macos"))]
                                    Ok(nix::sys::wait::WaitStatus::PtraceEvent(_,_,_)) => {

                                    }
                                    Err(_) => { return Err(jobs::Error::Wait); }
                                }
                            },
                            nix::sys::wait::WaitStatus::Exited(_,_) => {
                                break;
                            },
                            nix::sys::wait::WaitStatus::Signaled(_,_,_) => {
                                panic!("terminated job found in foreground queue");
                            },
                            #[cfg(not(target_os="macos"))]
                            nix::sys::wait::WaitStatus::PtraceEvent(_,_,_) => {
                                panic!("ptraced job found in foreground queue");
                            },
                            nix::sys::wait::WaitStatus::Stopped(_,_) => {
                                // bring job to foreground
                                break;
                            }
                        }
                    },
                }
            }
        }
        self.current_job_pid.write().unwrap().set(None);
        Ok(())
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
        match name {
            "cd" => {
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
            },
            "echo" => {
                println!("{}", args.join(" "));
                0
            },
            "echo-stderr" => {
                eprintln!("{}", args.join(" "));
                0
            },
            "exit" => {
                process::exit(0);
            },
            "set" => {
                if args.len() < 2 {
                    -1
                } else {
                    let var = &args[0];
                    let value = &args[1];
                    env::set_var(var, value);
                    0
                }
            },
            "jobs" => {
                println!("background: ");
                for (i, job) in self.background_jobs.read().unwrap().iter().enumerate() {
                    println!("  {}: {:?}", i, job);
                }
                println!("stopped: ");
                for (i, job) in self.stopped_jobs.read().unwrap().iter().enumerate() {
                    println!("  {}: {:?}", i, job);
                }
                0
            },
            "fg" | "bg" => {
                fn find_job_by_pid(jobs: &[jobs::Job], pid: nix::libc::pid_t) -> Option<usize> {
                    for (index, job) in jobs.iter().enumerate() {
                        match job.status.get() {
                            jobs::Status::Started(job_pid, _, _) => {
                                if job_pid == pid {
                                    return Some(index);
                                }
                            },
                            _ => {}
                        }
                    }
                    None
                }

                fn max_job_pid(jobs: &[jobs::Job]) -> Option<nix::libc::pid_t> {
                    let mut max_pid = None;
                    for job in jobs {
                        match job.status.get() {
                            jobs::Status::Started(job_pid, _, _) => {
                                if max_pid.is_none() {
                                    max_pid = Some(job_pid);
                                } else {
                                    if job_pid > max_pid.unwrap() {
                                        max_pid = Some(job_pid);
                                    }
                                }
                            },
                            _ => {}
                        }
                    }
                    max_pid
                }
                let mut bg_jobs = self.background_jobs.write().unwrap();
                let mut stopped_jobs = self.stopped_jobs.write().unwrap();
                let pid: Option<nix::libc::pid_t> = {
                    match args.first() {
                        Some(pid_str) => { pid_str.parse().ok() }
                        None => {
                            if name == "bg" {
                                max_job_pid(&bg_jobs)
                            } else {
                                match max_job_pid(&bg_jobs) {
                                    Some(max_bg_pid) => {
                                        match max_job_pid(&stopped_jobs) {
                                            Some(max_stopped_pid) => {
                                                if max_stopped_pid > max_bg_pid {
                                                    Some(max_stopped_pid)
                                                } else {
                                                    Some(max_bg_pid)
                                                }
                                            },
                                            None => {
                                                Some(max_bg_pid)
                                            }
                                        }
                                    },
                                    None => {
                                        max_job_pid(&stopped_jobs)
                                    }
                                }
                            }
                        }
                    }
                };
                if let Some(pid) = pid {
                    let job: jobs::Job;
                    match find_job_by_pid(&stopped_jobs, pid) {
                        Some(stopped_jobs_index) => {
                            job = stopped_jobs.remove(stopped_jobs_index);
                        },
                        None => {
                            if name == "bg" {
                                eprintln!("error: job {} has not stopped", pid);
                                return -1;
                            } else {
                                match find_job_by_pid(&bg_jobs, pid) {
                                    Some(bg_jobs_index) => {
                                        job = bg_jobs.remove(bg_jobs_index);
                                    },
                                    None => {
                                        eprintln!("error: no such job");
                                        return -1;
                                    }
                                }
                            }
                        }
                    };
                    self.foreground_jobs.write().unwrap().push(job);
                    0
                } else {
                    eprintln!("error: no such job");
                    -1
                }
            }
            _ => {
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
    }

    fn is_builtin(&mut self, name: &str) -> bool {
        let ketos_name = self.ketos_interp.scope().borrow_names_mut().add(name);
        if let Some(_) = self.ketos_interp.scope().get_value(ketos_name) {
            true
        } else {
            match name {
                "cd" | "echo" | "echo-stderr" | "exit" | "set" | "jobs" | "fg" | "bg" => {
                    true
                },
                _ => { false }
            }
        }
    }
}
