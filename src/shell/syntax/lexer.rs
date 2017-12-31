use nom::*;
use super::tokens::*;
use std::str::FromStr;

named!(pipe_operator<&str, Token>,
       do_parse!(tag!("|") >> (Token::Pipe))
);

named!(redirect_operator<&str, Token>,
       do_parse!(
           i: opt_res!(map_res!(digit, FromStr::from_str)) >>
           tag!(">") >> (Token::Redirect(i.unwrap_or(1)))
       )
);

named!(redirect_fd_operator<&str, Token>,
       do_parse!(
           i: opt_res!(map_res!(digit, FromStr::from_str)) >>
               tag!(">&") >>
               j: map_res!(digit, FromStr::from_str) >>(Token::RedirectFD(i.unwrap_or(1), j))
       )
);

named!(append_operator<&str, Token>,
       do_parse!(
           i: opt_res!(map_res!(digit, FromStr::from_str)) >>
           tag!(">>") >> (Token::Append(i.unwrap_or(1)))
       )
);

named!(redirectall_operator<&str, Token>,
       do_parse!(
           tag!("&>") >> (Token::RedirectAll)
       )
);

named!(appendall_operator<&str, Token>,
       do_parse!(
           tag!("&>>") >> (Token::AppendAll)
       )
);

named!(background_operator<&str, Token>,
       do_parse!(
           tag!("&") >> (Token::Background)
       )
);

named!(subshell_operator<&str, Token>,
       do_parse!(
           tag!("`") >> (Token::Subshell)
       )
);

named!(input_operator<&str, Token>,
       do_parse!(
           i: opt_res!(map_res!(digit, FromStr::from_str)) >>
               tag!("<") >> (Token::Input(i.unwrap_or(1)))
       )
);



named!(operator<&str, Token>,
       alt_complete!(
           ws!(pipe_operator) |
           ws!(append_operator) |
           ws!(redirect_fd_operator) |
           ws!(redirect_operator) |
           ws!(input_operator) |
           ws!(appendall_operator) |
           ws!(redirectall_operator) |
           ws!(background_operator) |
           ws!(subshell_operator)
       )
);

named!(quoted_string<&str, Token>,
       do_parse!(
           tag!("\"") >>
               i: quoted_string_inner >>
               tag!("\"") >>
               (i)
       )
);

#[derive(Debug, Clone)]
enum State {
    Base,
    ExpectVarOpen,
    VarOpen,
    VarClose,
    Escaped,
    InVar,
    HomeExpansion,
}

fn match_string<F>(input: &str, filter: F) -> IResult<&str, Token> where F: (Fn(char) -> bool) {
    let mut new_string_components: Vec<StringLiteralComponent> = Vec::new();
    let mut end_position = input.len();
    let mut state = State::Base;
    let mut last_base = 0;
    let mut last_var = 0;
    for (i, c) in input.char_indices() {
        match state {
            State::Base => {
                if filter(c) {
                    end_position = i;
                    break;
                }
                if c == '\\' {
                    if last_base < i {
                        new_string_components.push(StringLiteralComponent::Literal(&input[last_base..i]));
                    }
                    state = State::Escaped;
                }
                if c == '$' {
                    if last_base < i {
                        new_string_components.push(StringLiteralComponent::Literal(&input[last_base..i]));
                    }
                    state = State::ExpectVarOpen;
                }
                if c == '~' {
                    if last_base < i {
                        new_string_components.push(StringLiteralComponent::Literal(&input[last_base..i]));
                    }
                    new_string_components.push(StringLiteralComponent::EnvVar("HOME"));
                    state = State::HomeExpansion;
                }
            },
            State::HomeExpansion => {
                last_base = i;
                state = State::Base;
            }
            State::Escaped => {
                last_base = i;
                state = State::Base;
            },
            State::ExpectVarOpen => {
                if c == '{' {
                    state = State::VarOpen;
                } else {
                    return IResult::Error(ErrorKind::IsNot)
                }
            },
            State::VarOpen => {
                last_var = i;
                state = State::InVar;
            }
            State::InVar => {
                if c == '}' {
                    new_string_components.push(StringLiteralComponent::EnvVar(&input[last_var..i]));
                    state = State::VarClose;
                }
            },
            State::VarClose => {
                last_base = i;
                state = State::Base;
            }
        }
    }

    match state {
        State::Base => {
            if last_base < end_position {
                new_string_components.push(StringLiteralComponent::Literal(&input[last_base..end_position]));
            }
        },
        State::VarClose => {},
        State::HomeExpansion => {},
        _ => {
            return IResult::Error(ErrorKind::IsNot)
        }
    }

    let (head, tail) = input.split_at(end_position);
    if head.len() == 0 {
        IResult::Error(ErrorKind::LengthValue)
    } else {
        IResult::Done(tail, Token::StringLiteral(new_string_components))
    }
}

fn bare_string(input: &str) -> IResult<&str, Token> {
    match_string(input, |c| {
        char::is_whitespace(c) || c == '>' || c == '<' || c == '|' || c == '&' || c == '`'
    })
}

fn quoted_string_inner(input: &str) -> IResult<&str, Token> {
    match_string(input, |c| {
        c == '\"'
    })
}

/*
named!(bare_string<&str, Token>,
       do_parse!(
           s: take_till_s!(|c| {
               char::is_whitespace(c) || c == '>' || c == '<' || c == '|' || c == '&'
           }) >>
               (Token::StringLiteral(s.to_owned()))
       )
);
*/
/*
named!(bare_string_consume<&str, Token>,
       do_parse!(
           x: take_while_s!(&str, not_special_character)
           st: map!(many_till!(take_one_character, operator), |(s,c)| {s}) >> (Token::StringLiteral(st.iter().fold(String::new(), |acc, s| acc + s)))
       )
);
*/

named!(lex_one<&str, Token>,
       alt_complete!(
           ws!(operator) |
           ws!(quoted_string) |
           ws!(bare_string)
       )
);

named!(lex_all<&str, Vec<Token>>,
       ws!(many0!(lex_one))
);


pub fn lex<'a>(s: &'a str) -> IResult<&str, Vec<Token<'a>>> {
    lex_all(s)
}
