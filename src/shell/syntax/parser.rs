use super::tokens::*;
use super::ast::*;
use std::rc::Rc;

#[derive(Debug)]
pub enum Error {
    PipeConstruction,
    SubshellMatch,
    Subshell(Rc<Error>),
    ExpectedPath,
    ExpectedCommandName,
}

fn parse_one<'a>(t: &'a [Token]) -> Result<Expr<'a>, Error> {
    let mut arguments: Vec<Argument> = Vec::new();
    let mut iter = t.iter().enumerate();
    let binary: Option<&'a [StringLiteralComponent<'a>]> = {
        if let Some((_, &Token::StringLiteral(ref first))) = iter.next() {
            Some(first)
        } else {
            None
        }
    };
    while let Some((index, token)) = iter.next() {
        match token {
            &Token::StringLiteral(ref s) => arguments.push(Argument::Literal(s)),
            &Token::Pipe => {
                if let Ok(left) = parse_one(&t[0..index]) {
                    if let Ok(right) = parse_one(&t[index + 1..t.len()]) {
                        return Ok(Expr::Pipeline(Rc::new(left), Rc::new(right)));
                    } else {
                        return Err(Error::PipeConstruction);
                    }
                } else {
                    return Err(Error::PipeConstruction);
                }
            }
            &Token::Subshell => {
                iter.next();
                let mut next_index_opt = None;
                iter.clone().for_each(|(i, tk)| {
                    if tk == &Token::Subshell {
                        next_index_opt = Some(i);
                    }
                });
                if let Some(next_index) = next_index_opt {
                    let inner_result = parse_one(&t[index + 1..next_index]);
                    match inner_result {
                        Ok(inner) => {
                            arguments.push(Argument::Subshell(Rc::new(inner)));
                            while let Some(adv) = iter.next() {
                                if adv.0 > next_index {
                                    break;
                                }
                            }
                        }
                        Err(error) => {
                            return Err(Error::Subshell(Rc::new(error)));
                        }
                    }
                } else {
                    return Err(Error::SubshellMatch);
                }
            }
            &Token::Redirect(fd) => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Redirect(fd, target));
                } else {
                    return Err(Error::ExpectedPath);
                }
            }
            &Token::Append(fd) => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Append(fd, target));
                } else {
                    return Err(Error::ExpectedPath);
                }
            }
            &Token::RedirectAll => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Redirect(1, target));
                    arguments.push(Argument::Redirect(2, target));
                } else {
                    return Err(Error::ExpectedPath);
                }
            }
            &Token::AppendAll => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Append(1, target));
                    arguments.push(Argument::Append(2, target));
                } else {
                    return Err(Error::ExpectedPath);
                }
            }
            &Token::Input(fd) => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Input(fd, target));
                } else {
                    return Err(Error::ExpectedPath);
                }
            }
            &Token::RedirectFD(fd1, fd2) => {
                arguments.push(Argument::RedirectFD(fd1, fd2));
            }
            &Token::Background => {
                arguments.push(Argument::Background);
            }
        }
    }
    if let Some(b) = binary {
        Ok(Expr::Command(b, arguments))
    } else {
        Err(Error::ExpectedCommandName)
    }
}

pub fn parse<'a>(t: &'a [Token]) -> Result<Expr<'a>, Error> {
    parse_one(t)
}
