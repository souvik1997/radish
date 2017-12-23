#[derive(PartialEq, Debug, Clone)]
pub enum Token {
    StringLiteral(String),
    Pipe,
    Redirect(u32),
    RedirectFD(u32,u32),
    Append(u32),
    RedirectAll,
    AppendAll,
    Background,
    Input(u32),
    Subshell
}
