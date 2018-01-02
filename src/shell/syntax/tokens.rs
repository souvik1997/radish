use std::os::unix::io::RawFd;

#[derive(PartialEq, Debug, Clone)]
pub enum Token<'a> {
    StringLiteral(Vec<StringLiteralComponent<'a>>),
    Pipe,
    Redirect(RawFd),
    RedirectFD(RawFd, RawFd),
    Append(RawFd),
    RedirectAll,
    AppendAll,
    Background,
    Input(RawFd),
    Subshell,
}

#[derive(PartialEq, Debug, Clone)]
pub enum StringLiteralComponent<'a> {
    Literal(&'a str),
    EnvVar(&'a str),
    Brace(Vec<&'a str>),
}
