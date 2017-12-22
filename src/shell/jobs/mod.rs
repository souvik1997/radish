use std::process;
use nix;
use std::ffi::CString;
pub struct Job {
    pid: i32
}

impl Job {
    pub fn new(binary: &str, args: &[&str]) -> Option<Self> {

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

    }

    pub fn wait(&self) {
        nix::sys::wait::waitpid(self.pid, None);
    }
}
