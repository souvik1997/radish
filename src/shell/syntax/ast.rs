use super::tokens::Token;
use std::rc::Rc;

#[derive(PartialEq, Debug, Clone)]
pub enum Expr<'a> {
    Command(&'a str, Vec<&'a str>, Vec<OtherArgument<'a>>),
    Pipeline(Rc<Expr<'a>>, Rc<Expr<'a>>)
}

#[derive(PartialEq, Debug, Clone)]
pub enum OtherArgument<'a> {
    Redirect(u32, &'a str),
    RedirectFD(u32, u32),
    Append(u32, &'a str),
    Input(u32, &'a str),
    Background,
    Subshell(Rc<Expr<'a>>)
}
