use super::tokens::*;
use std::rc::Rc;

#[derive(PartialEq, Debug, Clone)]
pub enum Expr<'a> {
    Command(&'a [StringLiteralComponent<'a>], Vec<Argument<'a>>),
    Pipeline(Rc<Expr<'a>>, Rc<Expr<'a>>)
}

#[derive(PartialEq, Debug, Clone)]
pub enum Argument<'a> {
    Redirect(u32, &'a [StringLiteralComponent<'a>]),
    RedirectFD(u32, u32),
    Append(u32, &'a [StringLiteralComponent<'a>]),
    Input(u32, &'a [StringLiteralComponent<'a>]),
    Background,
    Subshell(Rc<Expr<'a>>),
    Literal(&'a [StringLiteralComponent<'a>]),
}
