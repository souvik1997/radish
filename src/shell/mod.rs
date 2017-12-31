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
                    let trimmed = command.trim();
                    if trimmed.starts_with("(") {
                        match self.state.ketos_interp.run_code(trimmed, None) {
                            Ok(value) => self.state.ketos_interp.display_value(&value),
                            Err(error) => println!("error: {}", error)
                        }
                    } else {
                        if let nom::IResult::Done(_, tokens) = syntax::lexer::lex(&command) {
                            println!("lexed: {:?}", tokens);
                            if let Some(expr) = syntax::parser::parse(&tokens) {
                                if let Ok(mut job) = jobs::Job::new(&expr) {
                                    job.run();
                                } else {
                                    println!("error when constructing job");
                                }

                            }
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
