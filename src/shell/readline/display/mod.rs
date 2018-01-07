use super::termion;
use std::fmt;
pub mod color;
use self::termion::color::Color as TermColor;
pub extern crate unicode_width;
use self::unicode_width::UnicodeWidthStr;

#[derive(Clone, Debug)]
pub struct Row<'a> {
    pub columns: Vec<Column<'a>>,
}

impl<'a> UnicodeWidthStr for Row<'a> {
    fn width(&self) -> usize {
        self.columns.iter().fold(0, |acc, s| {
            acc + s.width()
        })
    }
    fn width_cjk(&self) -> usize {
        self.columns.iter().fold(0, |acc, s| {
            acc + s.width_cjk()
        })
    }
}

#[derive(Clone, Debug)]
pub struct Column<'a> {
    pub left: DisplayString<'a>,
    pub center: DisplayString<'a>,
    pub right: DisplayString<'a>,
}

impl<'a> UnicodeWidthStr for Column<'a> {
    fn width(&self) -> usize {
        self.left.width() + self.center.width() + self.right.width()
    }

    fn width_cjk(&self) -> usize {
        self.left.width_cjk() + self.center.width_cjk() + self.right.width_cjk()
    }
}

#[derive(Clone, Debug)]
pub struct DisplayString<'a> {
    pub components: Vec<DisplayStringComponent<'a>>,
    pub cursor: Option<usize>,
}

impl<'a> DisplayString<'a> {
    pub fn new() -> DisplayString<'a> {
        DisplayString {
            components: Vec::new(),
            cursor: None,
        }
    }

    pub fn len(&self) -> usize {
        self.components.iter().fold(0, |acc, s| {
            acc + s.len()
        })
    }
}

impl<'a> UnicodeWidthStr for DisplayString<'a> {
    fn width(&self) -> usize {
        self.components.iter().fold(0, |acc, s| {
            acc + s.width()
        })
    }
    fn width_cjk(&self) -> usize {
        self.components.iter().fold(0, |acc, s| {
            acc + s.width_cjk()
        })
    }
}

impl<'a> ToString for DisplayString<'a> {
    fn to_string(&self) -> String {
        self.components.iter().fold(String::new(), |mut acc, s| {
            acc.push_str(&s.to_string());
            acc
        })
    }
}

impl<'a> ::std::convert::From<&'a str> for DisplayString<'a> {
    fn from(string: &'a str) -> DisplayString {
        DisplayString {
            components: vec![DisplayStringComponent::from(string)],
            cursor: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct DisplayStringComponent<'a> {
    pub text: &'a str,
    pub color: Color,
    pub style: Style,
    str_width: usize,
    str_width_cjk: usize,
}

impl<'a> DisplayStringComponent<'a> {
    pub fn new(text: &'a str, color: Color, style: Style) -> DisplayStringComponent<'a> {
        let width = text.width();
        let width_cjk = text.width_cjk();
        DisplayStringComponent {
            text: text,
            color: color,
            style: style,
            str_width: width,
            str_width_cjk: width_cjk,
        }
    }
    pub fn len(&self) -> usize {
        self.text.len()
    }
}

impl<'a> UnicodeWidthStr for DisplayStringComponent<'a> {
    fn width(&self) -> usize {
        self.str_width
    }
    fn width_cjk(&self) -> usize {
        self.str_width_cjk
    }
}

impl<'a> ToString for DisplayStringComponent<'a> {
    fn to_string(&self) -> String {
        String::from(self.text)
    }
}

impl<'a> ::std::convert::From<&'a str> for DisplayStringComponent<'a> {
    fn from(string: &'a str) -> DisplayStringComponent {
        DisplayStringComponent::new(
            string,
            Color::new(
                color::Mode::Normal(color::Base::Reset),
                color::Mode::Normal(color::Base::Reset),
            ),
            Style::NORMAL,
        )
    }
}

#[derive(Clone, Debug)]
pub struct Color {
    foreground: color::Mode,
    background: color::Mode,
}

impl Color {
    pub fn new(fg: color::Mode, bg: color::Mode) -> Color {
        Color {
            foreground: fg,
            background: bg,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let first = match &self.foreground {
            &color::Mode::Normal(ref col) => match col {
                &color::Base::Black => termion::color::Black.write_fg(f),
                &color::Base::Red => termion::color::Red.write_fg(f),
                &color::Base::Green => termion::color::Green.write_fg(f),
                &color::Base::Yellow => termion::color::Yellow.write_fg(f),
                &color::Base::Blue => termion::color::Blue.write_fg(f),
                &color::Base::Magenta => termion::color::Magenta.write_fg(f),
                &color::Base::Cyan => termion::color::Cyan.write_fg(f),
                &color::Base::White => termion::color::White.write_fg(f),
                &color::Base::Reset => termion::color::Reset.write_fg(f),
            },
            &color::Mode::Light(ref col) => match col {
                &color::Base::Black => termion::color::LightBlack.write_fg(f),
                &color::Base::Red => termion::color::LightRed.write_fg(f),
                &color::Base::Green => termion::color::LightGreen.write_fg(f),
                &color::Base::Yellow => termion::color::LightYellow.write_fg(f),
                &color::Base::Blue => termion::color::LightBlue.write_fg(f),
                &color::Base::Magenta => termion::color::LightMagenta.write_fg(f),
                &color::Base::Cyan => termion::color::LightCyan.write_fg(f),
                &color::Base::White => termion::color::LightWhite.write_fg(f),
                &color::Base::Reset => termion::color::Reset.write_fg(f),
            },
        };
        let second = match &self.background {
            &color::Mode::Normal(ref col) => match col {
                &color::Base::Black => termion::color::Black.write_bg(f),
                &color::Base::Red => termion::color::Red.write_bg(f),
                &color::Base::Green => termion::color::Green.write_bg(f),
                &color::Base::Yellow => termion::color::Yellow.write_bg(f),
                &color::Base::Blue => termion::color::Blue.write_bg(f),
                &color::Base::Magenta => termion::color::Magenta.write_bg(f),
                &color::Base::Cyan => termion::color::Cyan.write_bg(f),
                &color::Base::White => termion::color::White.write_bg(f),
                &color::Base::Reset => termion::color::Reset.write_bg(f),
            },
            &color::Mode::Light(ref col) => match col {
                &color::Base::Black => termion::color::LightBlack.write_bg(f),
                &color::Base::Red => termion::color::LightRed.write_bg(f),
                &color::Base::Green => termion::color::LightGreen.write_bg(f),
                &color::Base::Yellow => termion::color::LightYellow.write_bg(f),
                &color::Base::Blue => termion::color::LightBlue.write_bg(f),
                &color::Base::Magenta => termion::color::LightMagenta.write_bg(f),
                &color::Base::Cyan => termion::color::LightCyan.write_bg(f),
                &color::Base::White => termion::color::LightWhite.write_bg(f),
                &color::Base::Reset => termion::color::Reset.write_bg(f),
            },
        };
        first.and(second)
    }
}

bitflags! {
    pub struct Style: u8 {
        const BOLD = 0x1;
        const ITALIC = 0x2;
        const UNDERLINE = 0x4;
        const NORMAL = 0x0;
    }
}
impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut result = Ok(());
        if *self == Self::NORMAL {
            result = result.and(write!(f, "{}", termion::style::Reset));
        } else {
            if *self & Self::BOLD > Self::NORMAL {
                result = result.and(write!(f, "{}", termion::style::Bold));
            }
            if *self & Self::ITALIC > Self::NORMAL {
                result = result.and(write!(f, "{}", termion::style::Italic));
            }
            if *self & Self::UNDERLINE > Self::NORMAL {
                result = result.and(write!(f, "{}", termion::style::Underline));
            }
        }
        result
    }
}

pub trait Render {
    fn render<F: FnMut(&Row)>(&mut self, &mut F, width: usize);
}
