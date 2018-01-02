use super::tokens::*;
use std::rc::Rc;
use std::os::unix::io::RawFd;

#[derive(PartialEq, Debug, Clone)]
pub enum Expr<'a> {
    Command(&'a [StringLiteralComponent<'a>], Vec<Argument<'a>>),
    Pipeline(Rc<Expr<'a>>, Rc<Expr<'a>>),
}

#[derive(PartialEq, Debug, Clone)]
pub enum Argument<'a> {
    Redirect(RawFd, &'a [StringLiteralComponent<'a>]),
    RedirectFD(RawFd, RawFd),
    Append(RawFd, &'a [StringLiteralComponent<'a>]),
    Input(RawFd, &'a [StringLiteralComponent<'a>]),
    Background,
    Subshell(Rc<Expr<'a>>),
    Literal(&'a [StringLiteralComponent<'a>]),
}
