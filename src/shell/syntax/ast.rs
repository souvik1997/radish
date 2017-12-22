use super::tokens::Token;
use std::rc::Rc;

#[derive(PartialEq, Debug, Clone)]
pub enum Expr {
    Command(String, Vec<String>, Vec<OtherArgument>),
    Pipeline(Rc<Expr>, Rc<Expr>)
}

#[derive(PartialEq, Debug, Clone)]
pub enum OtherArgument {
    Redirect(u32, String),
    RedirectFD(u32, u32),
    Append(u32, String),
    Input(u32, String),
    Background
}
