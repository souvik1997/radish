mod readline;
mod syntax;
mod state;
mod completion;
mod history;
use self::state::ShellState;
use self::history::History;
use self::completion::Completer;
use nom;
use nix;
use std::io::stdin;
use std::os::unix::io::AsRawFd;

pub struct Shell {
    state: ShellState,
}

struct TerminalFgGroupManager {
    stdin_group: nix::unistd::Pid,
}

impl TerminalFgGroupManager {
    pub fn new(group: nix::unistd::Pid) -> Option<TerminalFgGroupManager> {
        match nix::unistd::tcgetpgrp(stdin().as_raw_fd()) {
            Ok(stdin_group) => {
                let t = TerminalFgGroupManager {
                    stdin_group: stdin_group,
                };
                nix::unistd::tcsetpgrp(stdin().as_raw_fd(), group)
                    .expect("failed to tcsetpgrp stdin");
                Some(t)
            }
            Err(_) => None,
        }
    }
}
impl Drop for TerminalFgGroupManager {
    fn drop(&mut self) {
        nix::unistd::tcsetpgrp(stdin().as_raw_fd(), self.stdin_group)
            .expect("failed to reset tcsetpgrp");
    }
}

struct ProcessGroupManager {
    group: nix::unistd::Pid,
}

impl ProcessGroupManager {
    pub fn new(group: nix::unistd::Pid) -> Option<ProcessGroupManager> {
        match nix::unistd::getpgid(None) {
            Ok(pgid) => {
                let t = ProcessGroupManager { group: pgid };
                match nix::unistd::setpgid(nix::unistd::Pid::this(), group) {
                    Ok(_) => Some(t),
                    Err(_) => None,
                }
            }
            Err(_) => None,
        }
    }
}

impl Drop for ProcessGroupManager {
    fn drop(&mut self) {
        nix::unistd::setpgid(nix::unistd::Pid::this(), self.group).expect("failed to restore process group");
    }
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            state: ShellState::new(),
        }
    }

    pub fn run_interactive(&mut self) -> i8 {
        let mut rl = readline::Readline::new();
        let history = History::new();
        let completer = Completer::new(&history);
        let state = &mut self.state;
        let pid = nix::unistd::getpid();
        let _process_group_manager =
            ProcessGroupManager::new(pid).expect("failed to set process group");
        let _terminal_group_manager =
            TerminalFgGroupManager::new(pid).expect("failed to set terminal process group");
        let block_sigaction = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::SigIgn,
            nix::sys::signal::SaFlags::empty(),
            nix::sys::signal::SigSet::empty(),
        );
        unsafe {
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGINT, &block_sigaction)
                .expect("failed to ignore SIGINT");
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGTSTP, &block_sigaction)
                .expect("failed to ignore SIGSTP");
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGQUIT, &block_sigaction)
                .expect("failed to ignore SIGQUIT");
        }
        let _reaper = state.start_background_reaper();
        loop {
            let input = rl.read(&completer, &history);
            match input {
                Some(command) => {
                    let trimmed = command.trim();
                    match syntax::lexer::lex(&trimmed) {
                        nom::IResult::Done(remaining, tokens) => {
                            if remaining.len() == 0 {
                                //println!("lexed: {:?}", tokens);
                                match syntax::parser::parse(&tokens) {
                                    Ok(expr) => match state.enqueue_job(&expr) {
                                        Ok(()) => {
                                            state
                                                .run_foreground_jobs()
                                                .expect("failed to run foreground jobs");
                                        }
                                        Err(error) => {
                                            println!("error when constructing job: {:?}", error);
                                        }
                                    },
                                    Err(error) => {
                                        println!("syntax error: {:?}", error);
                                    }
                                }
                            } else {
                                println!("syntax error: extraneous characters `{}`", remaining);
                            }
                        }
                        nom::IResult::Error(error) => {
                            println!("lex error: {:?}", error);
                        }
                        nom::IResult::Incomplete(nom::Needed::Unknown) => {
                            println!("lex error: incomplete input");
                        }
                        nom::IResult::Incomplete(nom::Needed::Size(remaining)) => {
                            println!("lex error: incomplete input, remaining: {}", remaining);
                        }
                    }
                }
                None => {
                    return 0;
                }
            }
        }
    }
}
