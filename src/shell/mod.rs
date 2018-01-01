mod readline;
mod syntax;
mod state;
use self::state::ShellState;
use nom;
use std::process;
use nix;

pub struct Shell {
    state: ShellState
}

struct TerminalFgGroupManager {
    stdin_group: nix::libc::pid_t,
    stdout_group: nix::libc::pid_t,
    stderr_group: nix::libc::pid_t,
}


impl TerminalFgGroupManager {
    pub fn new(group: nix::libc::pid_t) -> Option<TerminalFgGroupManager> {
        match nix::unistd::tcgetpgrp(0) {
            Ok(stdin_group) => {
                match nix::unistd::tcgetpgrp(1) {
                    Ok(stdout_group) => {
                        match nix::unistd::tcgetpgrp(2) {
                            Ok(stderr_group) => {
                                let t = TerminalFgGroupManager {
                                    stdin_group: stdin_group,
                                    stdout_group: stdout_group,
                                    stderr_group: stderr_group,
                                };
                                nix::unistd::tcsetpgrp(2, group);
                                nix::unistd::tcsetpgrp(1, group);
                                nix::unistd::tcsetpgrp(0, group);
                                Some(t)
                            },
                            Err(_) => None
                        }
                    },
                    Err(_) => None
                }
            },
            Err(_) => None
        }
    }
}
impl Drop for TerminalFgGroupManager {
    fn drop(&mut self) {
        nix::unistd::tcsetpgrp(2, self.stderr_group);
        nix::unistd::tcsetpgrp(1, self.stdout_group);
        nix::unistd::tcsetpgrp(0, self.stdin_group);
    }
}

struct ProcessGroupManager {
    group: nix::libc::pid_t
}

impl ProcessGroupManager {
    pub fn new(group: nix::libc::pid_t) -> Option<ProcessGroupManager> {
        match nix::unistd::getpgid(None) {
            Ok(pgid) => {
                let t = ProcessGroupManager {
                    group: pgid,
                };
                match nix::unistd::setpgid(0, group) {
                    Ok(_) => Some(t),
                    Err(_) => None
                }
            },
            Err(_) => None
        }
    }
}

impl Drop for ProcessGroupManager {
    fn drop(&mut self) {
        nix::unistd::setpgid(0, self.group).expect("failed to restore process group");
    }
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            state: ShellState::new()
        }
    }

    pub fn run_interactive(&mut self) {
        let state = &mut self.state;
        let pid = nix::unistd::getpid();
        let process_group_manager = ProcessGroupManager::new(pid).expect("failed to set process group");
        let terminal_group_manager = TerminalFgGroupManager::new(pid).expect("failed to set terminal process group");
        loop {
            let input = state.readline();
            match input {
                Ok(command) => {
                    let trimmed = command.trim();
                    if trimmed.starts_with("(") {
                        match state.ketos_interp.run_code(trimmed, None) {
                            Ok(value) => state.ketos_interp.display_value(&value),
                            Err(error) => println!("error: {}", error)
                        }
                    } else {
                        match syntax::lexer::lex(&command) {
                            nom::IResult::Done(remaining, tokens) =>  {
                                if remaining.len() == 0 {
                                    //println!("lexed: {:?}", tokens);
                                    match syntax::parser::parse(&tokens) {
                                        Ok(expr) => {
                                            match state.enqueue_job(&expr) {
                                                Ok(()) => {
                                                    state.run_foreground_jobs();
                                                },
                                                Err(error) => {
                                                    println!("error when constructing job: {:?}", error);
                                                }
                                            }
                                        },
                                        Err(error) => {
                                            println!("syntax error: {:?}", error);
                                        }
                                    }
                                } else {
                                    println!("syntax error: extraneous characters `{}`", remaining);
                                }
                            },
                            nom::IResult::Error(error) => {
                                println!("lex error: {:?}", error);
                            },
                            nom::IResult::Incomplete(nom::Needed::Unknown) => {
                                println!("lex error: incomplete input");
                            },
                            nom::IResult::Incomplete(nom::Needed::Size(remaining)) => {
                                println!("lex error: incomplete input, remaining: {}", remaining);
                            }
                        }
                    }
                }
                Err(error) => {
                    println!("error while reading input: {:?}", error);
                    process::exit(-1);
                }
            }
        }
    }
}
