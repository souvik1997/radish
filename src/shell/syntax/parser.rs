use nom::*;
use super::tokens::*;
use super::ast::*;
use std::str::FromStr;
use std::rc::Rc;

fn parse_one<'a>(t: &'a [Token]) -> Option<Expr<'a>> {
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
                if let Some(left) = parse_one(&t[0..index]) {
                    if let Some(right) = parse_one(&t[index+1..t.len()]) {
                        return Some(Expr::Pipeline(Rc::new(left), Rc::new(right)))
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            },
            &Token::Subshell => {
                iter.next();
                if let Some((next_index, _)) = iter.find(|&(_, tk)| {
                    tk == &Token::Subshell
                }) {
                    if let Some(inner) = parse_one(&t[index+1..next_index]) {
                        arguments.push(Argument::Subshell(Rc::new(inner)));
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }

            },
            &Token::Redirect(fd) => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Redirect(fd, target));
                } else {
                    return None;
                }
            },
            &Token::Append(fd) => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Append(fd, target));
                } else {
                    return None;
                }
            },
            &Token::RedirectAll => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Redirect(1, target));
                    arguments.push(Argument::Redirect(2, target));
                } else {
                    return None;
                }
            },
            &Token::AppendAll => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Append(1, target));
                    arguments.push(Argument::Append(2, target));
                } else {
                    return None;
                }
            },
            &Token::Input(fd) => {
                if let Some((_, &Token::StringLiteral(ref target))) = iter.next() {
                    arguments.push(Argument::Input(fd, target));
                } else {
                    return None;
                }
            },
            &Token::RedirectFD(fd1, fd2) => {
                arguments.push(Argument::RedirectFD(fd1, fd2));
            },
            &Token::Background => {
                arguments.push(Argument::Background);
            }
        }
    }
    if let Some(b) = binary {
        Some(Expr::Command(b, arguments))
    } else {
        None
    }
}

pub fn parse<'a>(t: &'a [Token]) -> Option<Expr<'a>> {
    parse_one(t)
}
