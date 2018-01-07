use super::syntax::ast::Expr;
use std::env;
extern crate users;
pub mod jobs;
use std::cell::Cell;
use std::path::PathBuf;
use std::ops::DerefMut;
use std::time;
use std::process;
use std::sync::RwLock;
use std::sync::Arc;
use std::thread;
use nix;
extern crate rlua;
use self::rlua::Lua;

pub struct ShellState {
    background_jobs: Arc<RwLock<Vec<jobs::Job>>>,
    foreground_jobs: Arc<RwLock<Vec<jobs::Job>>>,
    stopped_jobs: Arc<RwLock<Vec<jobs::Job>>>,
    current_job_pid: RwLock<Cell<Option<nix::unistd::Pid>>>,
    lua: RwLock<Lua>,
}

impl ShellState {
    pub fn new() -> Self {
        ShellState {
            background_jobs: Arc::new(RwLock::new(Vec::<jobs::Job>::new())),
            foreground_jobs: Arc::new(RwLock::new(Vec::<jobs::Job>::new())),
            stopped_jobs: Arc::new(RwLock::new(Vec::<jobs::Job>::new())),
            current_job_pid: RwLock::new(Cell::new(None)),
            lua: RwLock::new(Lua::new()),
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
                        }
                        Err(e) => Err(e),
                    }
                } else {
                    let mut foreground_jobs = self.foreground_jobs.write().unwrap();
                    foreground_jobs.push(job);
                    Ok(())
                }
            }
            Err(e) => Err(e),
        }
    }

    pub fn start_background_reaper(&self) -> thread::JoinHandle<()> {
        let background_jobs = self.background_jobs.clone();
        let stopped_jobs = self.stopped_jobs.clone();
        thread::spawn(move || loop {
            background_reaper(background_jobs.clone(), stopped_jobs.clone());
            thread::sleep(time::Duration::from_millis(500));
        })
    }

    pub fn run_foreground_jobs(&mut self) -> Result<(), jobs::Error> {
        while let Some(mut job) = get_next_job(&self.foreground_jobs) {
            self.current_job_pid.write().unwrap().set(None);
            loop {
                match job.get_status() {
                    jobs::Status::NotStarted => match job.run(self) {
                        Ok(_) => {
                            assert!(job.in_foreground());
                        }
                        Err(e) => {
                            return Err(e);
                        }
                    },
                    jobs::Status::Started(pid, _, status) => {
                        self.current_job_pid.write().unwrap().set(Some(pid));
                        if !job.in_foreground() {
                            job.set_foreground();
                        }
                        match status {
                            nix::sys::wait::WaitStatus::StillAlive
                            | nix::sys::wait::WaitStatus::Continued(_) => {
                                match job.wait(Some(nix::sys::wait::WUNTRACED)) {
                                    Ok(nix::sys::wait::WaitStatus::Stopped(_, _)) => {
                                        self.stopped_jobs.write().unwrap().push(job);
                                        break;
                                    }
                                    Ok(nix::sys::wait::WaitStatus::Exited(_, _))
                                    | Ok(nix::sys::wait::WaitStatus::Signaled(_, _, _)) => {
                                        break;
                                    }
                                    Ok(nix::sys::wait::WaitStatus::StillAlive) => {
                                        panic!("job should not be still running since waitpid was not called with WNOHANG");
                                    }
                                    Ok(nix::sys::wait::WaitStatus::Continued(_)) => {
                                        panic!("job should not be continued since waitpid was not called with WCONTINUED");
                                    }
                                    #[cfg(not(target_os = "macos"))]
                                    Ok(nix::sys::wait::WaitStatus::PtraceEvent(_, _, _)) => {}
                                    Err(_) => {
                                        return Err(jobs::Error::Wait);
                                    }
                                }
                            }
                            nix::sys::wait::WaitStatus::Exited(_, _) => {
                                break;
                            }
                            nix::sys::wait::WaitStatus::Signaled(_, _, _) => {
                                panic!("terminated job found in foreground queue");
                            }
                            #[cfg(not(target_os = "macos"))]
                            nix::sys::wait::WaitStatus::PtraceEvent(_, _, _) => {
                                panic!("ptraced job found in foreground queue");
                            }
                            nix::sys::wait::WaitStatus::Stopped(_, _) => {
                                job.cont(false).expect("failed to SIGCONT job");
                            }
                        }
                    }
                }
            }
        }
        self.current_job_pid.write().unwrap().set(None);
        Ok(())
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
            }
            "echo" => {
                println!("{}", args.join(" "));
                0
            }
            "echo-stderr" => {
                eprintln!("{}", args.join(" "));
                0
            }
            "exit" => {
                process::exit(0);
            }
            "set" => {
                if args.len() < 2 {
                    -1
                } else {
                    let var = &args[0];
                    let value = &args[1];
                    env::set_var(var, value);
                    0
                }
            }
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
            }
            "fg" | "bg" => {
                fn find_job_by_pid(jobs: &[jobs::Job], pid: nix::unistd::Pid) -> Option<usize> {
                    for (index, job) in jobs.iter().enumerate() {
                        match job.get_status() {
                            jobs::Status::Started(job_pid, _, _) => {
                                if job_pid == pid {
                                    return Some(index);
                                }
                            }
                            _ => {}
                        }
                    }
                    None
                }

                fn max_job_pid(jobs: &[jobs::Job]) -> Option<nix::unistd::Pid> {
                    let mut max_pid = None;
                    for job in jobs {
                        match job.get_status() {
                            jobs::Status::Started(job_pid, _, _) => {
                                if max_pid.is_none() {
                                    max_pid = Some(job_pid);
                                } else {
                                    if nix::libc::pid_t::from(job_pid) > nix::libc::pid_t::from(max_pid.unwrap()) {
                                        max_pid = Some(job_pid);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    max_pid
                }
                let mut bg_jobs = self.background_jobs.write().unwrap();
                let mut stopped_jobs = self.stopped_jobs.write().unwrap();
                let pid: Option<nix::unistd::Pid> = {
                    match args.first() {
                        Some(pid_str) => {
                            let parsed = pid_str.parse();
                            if parsed.is_ok() {
                                Some(nix::unistd::Pid::from_raw(parsed.unwrap()))
                            } else {
                                None
                            }
                        },
                        None => {
                            if name == "bg" {
                                max_job_pid(&stopped_jobs)
                            } else {
                                match max_job_pid(&bg_jobs) {
                                    Some(max_bg_pid) => match max_job_pid(&stopped_jobs) {
                                        Some(max_stopped_pid) => {
                                            if nix::libc::pid_t::from(max_stopped_pid) > nix::libc::pid_t::from(max_bg_pid) {
                                                Some(max_stopped_pid)
                                            } else {
                                                Some(max_bg_pid)
                                            }
                                        }
                                        None => Some(max_bg_pid),
                                    },
                                    None => max_job_pid(&stopped_jobs),
                                }
                            }
                        }
                    }
                };
                if let Some(pid) = pid {
                    let mut job: jobs::Job;
                    match find_job_by_pid(&stopped_jobs, pid) {
                        Some(stopped_jobs_index) => {
                            job = stopped_jobs.remove(stopped_jobs_index);
                        }
                        None => {
                            if name == "bg" {
                                eprintln!("error: job {} has not stopped", pid);
                                return -1;
                            } else {
                                match find_job_by_pid(&bg_jobs, pid) {
                                    Some(bg_jobs_index) => {
                                        job = bg_jobs.remove(bg_jobs_index);
                                    }
                                    None => {
                                        eprintln!("error: no such job");
                                        return -1;
                                    }
                                }
                            }
                        }
                    };
                    if name == "fg" {
                        self.foreground_jobs.write().unwrap().push(job);
                    } else {
                        job.cont(true)
                            .expect("failed to continue job in background");
                        bg_jobs.push(job);
                    }
                    0
                } else {
                    eprintln!("error: no such job");
                    -1
                }
            }
            _ => -1,
        }
    }

    fn is_builtin(&mut self, name: &str) -> bool {
        match name {
            "cd" | "echo" | "echo-stderr" | "exit" | "set" | "jobs" | "fg" | "bg" => true,
            _ => false,
        }
    }
}

fn get_next_job(queue: &RwLock<Vec<jobs::Job>>) -> Option<jobs::Job> {
    let mut foreground_jobs = queue.write().unwrap();
    if foreground_jobs.len() > 0 {
        let job = foreground_jobs.remove(0);
        Some(job)
    } else {
        None
    }
}

fn background_reaper(
    background_jobs: Arc<RwLock<Vec<jobs::Job>>>,
    stopped_jobs: Arc<RwLock<Vec<jobs::Job>>>,
) {
    let mut bg_jobs = background_jobs.write().unwrap();
    let mut st_jobs = stopped_jobs.write().unwrap();
    let mut new_bg_jobs = Vec::new();
    while let Some(mut job) = bg_jobs.pop() {
        match job.get_status() {
            jobs::Status::NotStarted => {
                panic!("found job in bg queue that has not been started");
            }
            jobs::Status::Started(_, _, status) => {
                match status {
                    nix::sys::wait::WaitStatus::StillAlive
                    | nix::sys::wait::WaitStatus::Continued(_) => {
                        match job.wait(Some(nix::sys::wait::WUNTRACED | nix::sys::wait::WNOHANG)) {
                            Ok(_) => {
                                new_bg_jobs.push(job);
                            }
                            Err(_) => {} // silently drop and kill job
                        }
                    }
                    nix::sys::wait::WaitStatus::Stopped(_, _) => {
                        st_jobs.push(job);
                    }
                    _ => {} // remove from queue
                }
            }
        }
    }
    *bg_jobs.deref_mut() = new_bg_jobs;
}
