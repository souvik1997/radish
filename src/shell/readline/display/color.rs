#[derive(Clone, Debug)]
pub enum Base {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    Reset,
}

#[derive(Clone, Debug)]
pub enum Mode {
    Normal(Base),
    Light(Base),
}
