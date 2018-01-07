mod readline;
use self::readline::Readline;
mod syntax;
mod jobs;
use self::jobs::JobManager;
mod completion;
use self::completion::Completer;
mod history;
use self::history::History;
use nom;
use nix;
use std::io::stdin;
use std::os::unix::io::AsRawFd;
extern crate rlua;
use self::rlua::Lua;

pub struct Shell {}

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
                nix::unistd::tcsetpgrp(stdin().as_raw_fd(), group).expect("failed to tcsetpgrp stdin");
                Some(t)
            }
            Err(_) => None,
        }
    }
}
impl Drop for TerminalFgGroupManager {
    fn drop(&mut self) {
        nix::unistd::tcsetpgrp(stdin().as_raw_fd(), self.stdin_group).expect("failed to reset tcsetpgrp");
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
    pub fn run() -> i8 {
        // Set up subsystems
        let mut history = History::new("history.sqlite").expect("failed to open history file");
        let mut lua = Lua::new();
        let mut completion_engines: Vec<Box<completion::engines::Engine>> = vec![
            Box::new(completion::engines::PathCompletion::new()),
            Box::new(completion::engines::UserCompletion::new(&lua, "name")),
        ];
        let mut completer = Completer::new(completion_engines);
        let mut job_manager = JobManager::new();
        let mut readline = Readline::new();
        Shell::run_interactive(
            &mut history,
            &mut completer,
            &mut job_manager,
            &mut readline,
        )
    }

    fn run_interactive<'a, 'b: 'a>(mut history: &'a mut History, mut completer: &'a mut Completer<'b>, mut job_manager: &mut JobManager, mut readline: &mut Readline) -> i8 {
        let pid = nix::unistd::getpid();
        let _process_group_manager = ProcessGroupManager::new(pid).expect("failed to set process group");
        let _terminal_group_manager = TerminalFgGroupManager::new(pid).expect("failed to set terminal process group");
        let block_sigaction = nix::sys::signal::SigAction::new(
            nix::sys::signal::SigHandler::SigIgn,
            nix::sys::signal::SaFlags::empty(),
            nix::sys::signal::SigSet::empty(),
        );
        unsafe {
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGINT, &block_sigaction).expect("failed to ignore SIGINT");
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGTSTP, &block_sigaction).expect("failed to ignore SIGSTP");
            nix::sys::signal::sigaction(nix::sys::signal::Signal::SIGQUIT, &block_sigaction).expect("failed to ignore SIGQUIT");
        }
        job_manager.start_background_reaper();
        let result;
        loop {
            let input = readline.read(&mut completer, &history);
            match input {
                Some(command) => {
                    let trimmed = command.trim();
                    history
                        .add_command(&trimmed)
                        .expect("failed to add command to history");
                    match syntax::lexer::lex(&trimmed) {
                        nom::IResult::Done(remaining, tokens) => {
                            if remaining.len() == 0 {
                                //println!("lexed: {:?}", tokens);
                                match syntax::parser::parse(&tokens) {
                                    Ok(expr) => match job_manager.enqueue_job_from_expr(&expr) {
                                        Ok(()) => {
                                            job_manager
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
                    result = 0;
                    break;
                }
            }
        }
        // we don't need to stop the reaper because it's not owned by the job manager, and we're exiting anyway
        result
    }
}
