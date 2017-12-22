use std::sync::Arc;

mod readline;
mod syntax;
mod state;
mod jobs;
use self::state::ShellState;
use nom;

pub struct Shell {
    readline: readline::Editor<Arc<ShellState>>,
    state: Arc<ShellState>
}

impl Shell {
    pub fn new() -> Self {
        let state = Arc::new(ShellState::new());
        let readline = readline::Editor::<Arc<ShellState>>::new(Arc::clone(&state));
        Shell {
            readline: readline,
            state: state
        }
    }

    pub fn run_interactive(&mut self) {
        loop {
            let input = self.readline.readline();
            match input {
                Ok(command) => {
                    println!("Got command {}", command);
                    if let nom::IResult::Done(_, tokens) = syntax::lexer::lex(&command) {
                        println!("{:?}", syntax::parser::parse(&tokens));
                    }
                    // TODO: use tokenized output to construct argv
                    let split: Vec<&str> = command.split_whitespace().collect();
                    if let Some(binary) = split.first() {
                        if let Some(job) = jobs::Job::new(&binary, &split) {
                            job.wait();
                        }
                    }
                }
                Err(_) => {
                    println!("Error when reading input!");
                }
            }
        }
    }
}
