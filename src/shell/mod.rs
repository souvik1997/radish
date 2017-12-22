use std::sync::Arc;

mod readline;
mod syntax;
mod state;
use self::state::ShellState;

pub struct Shell {
    readline: readline::Editor<Arc<ShellState>>,
    state: Arc<ShellState>
}

impl Shell {
    pub fn new() -> Self {
        let state = Arc::new(ShellState::new());
        let mut readline = readline::Editor::<Arc<ShellState>>::new();
        readline.set_completer(Some(Arc::clone(&state)));
        Shell {
            readline: readline,
            state: state
        }
    }

    pub fn run_interactive(&mut self) {
        loop {
            let input = self.readline.readline("> ");
            match input {
                Ok(command) => {
                    println!("Got command {}", command);
                    syntax::lexer::test_lex(&command);
                }
                Err(_) => {
                    println!("Error when reading input!");
                }
            }
        }
    }
}
