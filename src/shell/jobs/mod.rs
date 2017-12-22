use std::process;
use super::super::sys;
pub struct Job {
    pid: u32
}

impl Job {
    pub fn new(binary: &str, args: &[&str]) -> Option<Self> {
        if let Ok(fork_result) = unsafe {sys::fork()} {
            if fork_result == 0 {
                match sys::execve(binary, args, false) {
                    Err(_) => process::exit(-1),
                    Ok(_) => unreachable!()
                }
            } else {
                Some(Job { pid: fork_result })
            }
        } else {
            None
        }
    }

    pub fn wait(&self) {
        sys::waitpid(self.pid, 0);
    }
}
