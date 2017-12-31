mod readline;
mod syntax;
mod state;
use self::state::ShellState;
use nom;
use std::process;

pub struct Shell {
    state: ShellState
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            state: ShellState::new()
        }
    }

    pub fn run_interactive(&mut self) {
        let state = &mut self.state;
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
                                            match state.run_job(&expr) {
                                                Ok(ref mut job) => {
                                                    match job.wait_until_complete() {
                                                        Ok(_) => {},
                                                        Err(error) => println!("error while running `{}`: {}", command, error)
                                                    }
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
