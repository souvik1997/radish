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
                        println!("lexed: {:?}", tokens);
                        if let Some(expr) = syntax::parser::parse(&tokens) {
                            self.evaluate(&expr);
                        }
                    }
                }
                Err(_) => {
                    println!("Error when reading input!");
                }
            }
        }
    }

    fn evaluate(&mut self, expr: &syntax::ast::Expr) {
        println!("expr: {:?}", expr);
        match expr {
            &syntax::ast::Expr::Command(ref binary, ref arguments, ref other_arguments) => {
                if other_arguments.len() > 0 {
                    println!("Other arguments (not implemented): {:?}", other_arguments);
                }
                if let Some(job) = jobs::Job::new(&binary, &arguments) {
                    job.wait();
                }
            }
            &syntax::ast::Expr::Pipeline(_, _) => {
                println!("pipelines not implemented!");
            }
        }
    }
}
