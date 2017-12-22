use nom::*;
use super::tokens::*;
use super::ast::*;
use std::str::FromStr;
use std::rc::Rc;

fn parse_one<'a>(t: &'a [Token]) -> Option<Expr> {
    let mut arguments: Vec<&'a str> = Vec::new();
    let mut other_arguments: Vec<OtherArgument> = Vec::new();
    let mut iter = t.iter();
    let binary: Option<&'a str> = {
        if let Some(&Token::StringLiteral(ref first)) = iter.next() {
            Some(first)
        } else {
            None
        }
    };
    let mut index = 1;
    while let Some(token) = iter.next() {
        match token {
            &Token::StringLiteral(ref s) => arguments.push(s),
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
            &Token::Redirect(fd) => {
                if let Some(&Token::StringLiteral(ref target)) = iter.next() {
                    other_arguments.push(OtherArgument::Redirect(fd, target));
                    index += 1;
                } else {
                    return None;
                }
            },
            &Token::Append(fd) => {
                if let Some(&Token::StringLiteral(ref target)) = iter.next() {
                    other_arguments.push(OtherArgument::Append(fd, target));
                    index += 1;
                } else {
                    return None;
                }
            },
            &Token::RedirectAll => {
                if let Some(&Token::StringLiteral(ref target)) = iter.next() {
                    other_arguments.push(OtherArgument::Redirect(1, target));
                    other_arguments.push(OtherArgument::Redirect(2, target));
                    index += 1;
                } else {
                    return None;
                }
            },
            &Token::AppendAll => {
                if let Some(&Token::StringLiteral(ref target)) = iter.next() {
                    other_arguments.push(OtherArgument::Append(1, target));
                    other_arguments.push(OtherArgument::Append(2, target));
                    index += 1;
                } else {
                    return None;
                }
            },
            &Token::Input(fd) => {
                if let Some(&Token::StringLiteral(ref target)) = iter.next() {
                    other_arguments.push(OtherArgument::Input(fd, target));
                    index += 1;
                } else {
                    return None;
                }
            },
            &Token::RedirectFD(fd1, fd2) => {
                other_arguments.push(OtherArgument::RedirectFD(fd1, fd2));
            },
            &Token::Background => {
                other_arguments.push(OtherArgument::Background);
            },
        }
    }
    if let Some(b) = binary {
        Some(Expr::Command(b, arguments, other_arguments))
    } else {
        None
    }
}

pub fn parse(t: &[Token]) -> Option<Expr> {
    parse_one(t)
}
