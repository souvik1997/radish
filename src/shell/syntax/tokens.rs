#[derive(PartialEq, Debug, Clone)]
pub enum Token<'a> {
    StringLiteral(Vec<StringLiteralComponent<'a>>),
    Pipe,
    Redirect(u32),
    RedirectFD(u32,u32),
    Append(u32),
    RedirectAll,
    AppendAll,
    Background,
    Input(u32),
    Subshell,
}

#[derive(PartialEq, Debug, Clone)]
pub enum StringLiteralComponent<'a> {
    Literal(&'a str),
    EnvVar(&'a str)
}
