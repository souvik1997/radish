// Source: https://github.com/redox-os/ion/blob/adde38d82bca7ad5595db57898e75694cb3278ac/src/sys/unix/mod.rs

use libc;
use libc::{c_char, c_int, pid_t, sighandler_t};
use std::{io, ptr};
use std::env;
use std::ffi::CString;
use std::os::unix::io::RawFd;

pub(crate) const PATH_SEPARATOR: &str = ":";

pub(crate) const O_CLOEXEC: usize = libc::O_CLOEXEC as usize;
pub(crate) const SIGHUP: i32 = libc::SIGHUP;
pub(crate) const SIGINT: i32 = libc::SIGINT;
pub(crate) const SIGTERM: i32 = libc::SIGTERM;
pub(crate) const SIGCONT: i32 = libc::SIGCONT;
pub(crate) const SIGSTOP: i32 = libc::SIGSTOP;
pub(crate) const SIGTSTP: i32 = libc::SIGTSTP;

pub(crate) const STDOUT_FILENO: i32 = libc::STDOUT_FILENO;
pub(crate) const STDERR_FILENO: i32 = libc::STDERR_FILENO;
pub(crate) const STDIN_FILENO: i32 = libc::STDIN_FILENO;

pub(crate) fn getlogin() -> Option<String> {
    if let Ok(username) = unsafe{ CString::from_raw(libc::getlogin()) }.into_string() {
        Some(username)
    } else {
        None
    }
}

pub(crate) fn geteuid() -> io::Result<u32> { Ok(unsafe { libc::geteuid() } as u32) }

pub(crate) fn getuid() -> io::Result<u32> { Ok(unsafe { libc::getuid() } as u32) }

pub(crate) fn is_root() -> bool { unsafe { libc::geteuid() == 0 } }

pub unsafe fn fork() -> io::Result<u32> { cvt(libc::fork()).map(|pid| pid as u32) }

pub(crate) fn getpid() -> io::Result<u32> { cvt(unsafe { libc::getpid() }).map(|pid| pid as u32) }

pub(crate) fn kill(pid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(pid as pid_t, signal as c_int) }).and(Ok(()))
}

pub(crate) fn killpg(pgid: u32, signal: i32) -> io::Result<()> {
    cvt(unsafe { libc::kill(-(pgid as pid_t), signal as c_int) }).and(Ok(()))
}

pub(crate) fn waitpid(pid: u32, opts: i32) -> io::Result<(i32, i32)> {
    let mut status = 0;
    let pid = cvt(unsafe { libc::waitpid(pid as pid_t, &mut status, opts) });
    if pid.is_err() {
        Err(pid.err().unwrap())
    } else {
        Ok((pid.ok().unwrap(), status))
    }
}

pub(crate) fn execve(prog: &str, args: &[&str], clear_env: bool) -> io::Result<()> {
    // Prepare the program string
    let prog_str = match CString::new(prog) {
        Ok(prog_str) => prog_str,
        Err(_) => {
            return Err(io::Error::last_os_error());
        }
    };

    // Create the arguments vector
    let mut cvt_args: Vec<CString> = Vec::new();
    cvt_args.push(prog_str.clone());
    for arg in args.iter() {
        match CString::new(*arg) {
            Ok(arg) => cvt_args.push(arg),
            Err(_) => {
                return Err(io::Error::last_os_error());
            }
        }
    }
    let mut arg_ptrs: Vec<*const c_char> = cvt_args.iter().map(|x| x.as_ptr()).collect();
    // NULL terminate the argv array
    arg_ptrs.push(ptr::null());

    // Get the PathBuf of the program if it exists.
    let prog = if prog.contains('/') {
        // This is a fully specified path to an executable.
        Some(prog_str)
    } else if let Ok(paths) = env::var("PATH") {
        // This is not a fully specified scheme or path.
        // Iterate through the possible paths in the
        // env var PATH that this executable may be found
        // in and return the first one found.
        env::split_paths(&paths)
            .filter_map(|mut path| {
                path.push(prog);
                match (path.exists(), path.to_str()) {
                    (false, _) => None,
                    (true, Some(path)) => match CString::new(path) {
                        Ok(prog_str) => Some(prog_str),
                        Err(_) => None,
                    },
                    (true, None) => None,
                }
            })
            .next()
    } else {
        None
    };

    let mut env_ptrs: Vec<*const c_char> = Vec::new();
    let mut env_vars: Vec<CString> = Vec::new();
    // If clear_env is not specified build envp
    if !clear_env {
        for (key, value) in env::vars() {
            match CString::new(format!("{}={}", key, value)) {
                Ok(var) => env_vars.push(var),
                Err(_) => {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        env_ptrs = env_vars.iter().map(|x| x.as_ptr()).collect();
    }
    env_ptrs.push(ptr::null());

    if let Some(prog) = prog {
        // If we found the program. Run it!
        cvt(unsafe { libc::execve(prog.as_ptr(), arg_ptrs.as_ptr(), env_ptrs.as_ptr()) })
            .and(Ok(()))
    } else {
        // The binary was not found.
        Err(io::Error::from_raw_os_error(libc::ENOENT))
    }
}

pub(crate) fn pipe2(_flags: usize) -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0; 2];

    #[cfg(not(target_os = "macos"))]
    cvt(unsafe { libc::pipe2(fds.as_mut_ptr(), _flags as c_int) })?;

    #[cfg(target_os = "macos")]
    cvt(unsafe { libc::pipe(fds.as_mut_ptr()) })?;

    Ok((fds[0], fds[1]))
}

pub(crate) fn setpgid(pid: u32, pgid: u32) -> io::Result<()> {
    cvt(unsafe { libc::setpgid(pid as pid_t, pgid as pid_t) }).and(Ok(()))
}

#[allow(dead_code)]
pub(crate) fn signal(signal: i32, handler: extern "C" fn(i32)) -> io::Result<()> {
    if unsafe { libc::signal(signal as c_int, handler as sighandler_t) } == libc::SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn reset_signal(signal: i32) -> io::Result<()> {
    if unsafe { libc::signal(signal as c_int, libc::SIG_DFL) } == libc::SIG_ERR {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn tcsetpgrp(fd: RawFd, pgrp: u32) -> io::Result<()> {
    cvt(unsafe { libc::tcsetpgrp(fd as c_int, pgrp as pid_t) }).and(Ok(()))
}

pub(crate) fn dup(fd: RawFd) -> io::Result<RawFd> { cvt(unsafe { libc::dup(fd) }) }

pub(crate) fn dup2(old: RawFd, new: RawFd) -> io::Result<RawFd> {
    cvt(unsafe { libc::dup2(old, new) })
}

pub(crate) fn close(fd: RawFd) -> io::Result<()> { cvt(unsafe { libc::close(fd) }).and(Ok(())) }

pub(crate) fn isatty(fd: RawFd) -> bool { unsafe { libc::isatty(fd) == 1 } }

trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

macro_rules! impl_is_minus_one {
        ($($t:ident)*) => ($(impl IsMinusOne for $t {
            fn is_minus_one(&self) -> bool {
                *self == -1
            }
        })*)
    }

impl_is_minus_one! { i8 i16 i32 i64 isize }

fn cvt<T: IsMinusOne>(t: T) -> io::Result<T> {
    if t.is_minus_one() {
        Err(io::Error::last_os_error())
    } else {
        Ok(t)
    }
}
