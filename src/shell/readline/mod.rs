extern crate nix;
extern crate termion;
mod editor;
use self::editor::Editor;
mod display;
use self::display::*;
use self::unicode_width::UnicodeWidthStr;
mod line_editor;
use std::io::{self, stdin, stdout, Write};
use std::fmt;
use std::cmp::max;
use super::history::History;
use super::completion::Completer;
use self::termion::input::TermRead;
use self::termion::raw::IntoRawMode;

pub struct Readline {}

pub enum ReadlineEvent {
    ClearScreen,
    Done,
    Eof,
    Interrupted,
    HistorySearch,
    Continue,
    StartCompletionPager,
}

impl Readline {
    pub fn new() -> Readline {
        Readline {}
    }

    pub fn read(&mut self, completer: &Completer, history: &History) -> Option<String> {
        let result;
        loop {
            let res = self.read_impl(completer, history);
            println!("");
            match res {
                Ok(string) => {
                    result = Some(string);
                    break;
                }
                Err(event) => match event {
                    ReadlineEvent::ClearScreen => {}
                    ReadlineEvent::Eof => {
                        return None;
                    }
                    ReadlineEvent::Interrupted => {}
                    _ => {
                        panic!("unexpected event");
                    }
                },
            }
        }
        result
    }

    fn render(
        &self,
        editor: &mut Editor,
        cursor: &mut CursorManager,
        term_buffer: &mut TerminalBuffer,
        terminal_width: usize,
        mut stdout: &mut Write,
    ) {
        term_buffer.set_width(terminal_width);
        let new_position = {
            let mut index = 0;
            let mut cursor_position: Option<(u16, u16)> = None;
            editor.render(
                &mut |ref row| {
                    if let Some(row_cursor_position) =
                        term_buffer.add_row(row).expect("failed to write to stdout")
                    {
                        assert!(cursor_position.is_none());
                        cursor_position = Some((row_cursor_position as u16, index as u16))
                    }
                    index += 1;
                },
                terminal_width,
            );
            cursor_position
        };
        cursor.set_hidden(&mut stdout, true);
        cursor.move_to(&mut stdout, 0, 0);
        term_buffer
            .render(&mut stdout)
            .expect("failed to write buffer to stdout");
        if let Some((x, y)) = new_position {
            cursor.move_to(&mut stdout, x, y);
            cursor.set_hidden(&mut stdout, false);
        } else {
            cursor.move_to(&mut stdout, 0, 0);
            cursor.set_hidden(&mut stdout, true);
        }
        stdout.flush().expect("failed to flush stdout");
    }

    fn read_impl(
        &mut self,
        completer: &Completer,
        history: &History,
    ) -> Result<String, ReadlineEvent> {
        let mut editor = Editor::new(completer, history);
        let mut term_buffer = TerminalBuffer::new();
        let mut stdout = stdout().into_raw_mode().expect("failed to set raw mode");
        let mut cursor = CursorManager::new();
        let (mut terminal_width, mut terminal_height) =
            termion::terminal_size().expect("failed to get terminal size");
        let mut result = Err(ReadlineEvent::Interrupted);
        let stdin = stdin();
        self.render(
            &mut editor,
            &mut cursor,
            &mut term_buffer,
            terminal_width as usize,
            &mut stdout,
        );
        for key in stdin.keys() {
            // input
            if let Ok(key) = key {
                match editor.handle_input(key) {
                    ReadlineEvent::Continue => {}
                    ReadlineEvent::Done => {
                        result = Ok(editor.buffer());
                        break;
                    }
                    other => {
                        result = Err(other);
                        break;
                    }
                }
                let (new_terminal_width, new_terminal_height) =
                    termion::terminal_size().expect("failed to get terminal size");
                if new_terminal_width != terminal_width || new_terminal_height != terminal_height {
                    terminal_width = new_terminal_width;
                    terminal_height = new_terminal_height;
                }
            } else {
                break;
            }
            // render
            self.render(
                &mut editor,
                &mut cursor,
                &mut term_buffer,
                terminal_width as usize,
                &mut stdout,
            );
        }
        cursor.move_to(&mut stdout, 0, 0);
        self.render(
            &mut editor,
            &mut cursor,
            &mut term_buffer,
            terminal_width as usize,
            &mut stdout,
        );
        cursor.move_to(&mut stdout, 0, 0);
        result
    }
}

struct CursorManager {
    xpos: u16,
    ypos: u16,
    hidden: bool,
}

impl CursorManager {
    pub fn new() -> CursorManager {
        CursorManager {
            xpos: 0,
            ypos: 0,
            hidden: false,
        }
    }

    pub fn position(&self) -> (u16, u16) {
        (self.xpos, self.ypos)
    }

    pub fn is_hidden(&self) -> bool {
        self.hidden
    }

    pub fn set_hidden<W: io::Write>(&mut self, writer: &mut W, hidden: bool) {
        self.hidden = hidden;
        let err_msg = "failed to write to stdout";
        if hidden {
            write!(writer, "{}", termion::cursor::Hide).expect(&err_msg);
        } else {
            write!(writer, "{}", termion::cursor::Show).expect(&err_msg);
        }
    }

    pub fn move_to<W: io::Write>(&mut self, writer: &mut W, x: u16, y: u16) {
        let err_msg = "failed to write to stdout";
        if x < self.xpos {
            write!(writer, "{}", termion::cursor::Left(self.xpos - x)).expect(&err_msg);
        } else if x > self.xpos {
            write!(writer, "{}", termion::cursor::Right(x - self.xpos)).expect(&err_msg);
        }

        if y < self.ypos {
            write!(writer, "{}", termion::cursor::Up(self.ypos - y)).expect(&err_msg);
        } else if y > self.ypos {
            write!(writer, "{}", termion::cursor::Down(y - self.ypos)).expect(&err_msg);
        }
        self.xpos = x;
        self.ypos = y;
    }
}

struct TerminalBuffer {
    buffer: Vec<String>,
    last_rendered: Vec<String>,
    terminal_width: usize,
}

impl TerminalBuffer {
    pub fn new() -> TerminalBuffer {
        TerminalBuffer {
            buffer: Vec::new(),
            last_rendered: Vec::new(),
            terminal_width: 80,
        }
    }

    pub fn set_width(&mut self, width: usize) {
        self.terminal_width = width;
    }

    pub fn render(&mut self, writer: &mut Write) -> io::Result<()> {
        // precondition: cursor is at (0,0)
        let num_lines = self.buffer.len();
        for (index, line) in self.buffer.iter().enumerate() {
            let can_reuse_line = {
                if let Some(previous_render) = self.last_rendered.get(index) {
                    if previous_render == line {
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            };
            if !can_reuse_line {
                write!(writer, "{}", termion::clear::CurrentLine)?;
                write!(writer, "{}", line)?;
            }
            write!(writer, "\r\n")?;
        }
        write!(writer, "{}", termion::cursor::Up(num_lines as u16))?;
        self.last_rendered.clear();
        self.last_rendered.append(&mut self.buffer);
        // postcondition: cursor is at (0,0)
        Ok(())
    }

    pub fn add_row(&mut self, row: &Row) -> Result<Option<usize>, fmt::Error> {
        use std::fmt::Write;
        let mut output = String::new();
        write!(
            output,
            "{}{}",
            Color::new(
                color::Mode::Normal(color::Base::Reset),
                color::Mode::Normal(color::Base::Reset)
            ),
            Style::NORMAL
        )?;
        let mut cursor_position = None;
        let row_width = {
            if row.columns.len() == 1 {
                self.terminal_width
            } else {
                max(120, self.terminal_width)
            }
        };
        assert!(row.width() <= self.terminal_width);
        let total_cols = row.columns.len();
        let mut remaining_width = row_width;
        for (col_index, col) in row.columns.iter().enumerate() {
            let column_width = remaining_width / (total_cols - col_index);
            remaining_width -= column_width;
            let begin = col_index * column_width;
            let end = col_index * column_width + column_width;
            let (left_start, center_start, right_start) = {
                if col.center.width() == 0 && col.right.width() == 0 {
                    (begin, end, end)
                } else if col.center.width() == 0 && col.right.width() > 0 {
                    (begin, begin + col.left.width(), end - col.right.width())
                } else if col.center.width() > 0 && col.right.width() == 0 {
                    (
                        begin,
                        begin + col.left.width() + (column_width - col.left.width()) / 2,
                        end,
                    )
                } else if col.center.width() > 0 && col.right.width() > 0 {
                    let r = end - col.right.width();
                    (begin, begin + (r - begin - col.center.width()) / 2, r)
                } else {
                    panic!("cannot reach case")
                }
            };
            let left_end = left_start + col.left.width();
            let center_end = center_start + col.center.width();
            let right_end = right_start + col.right.width();
            assert!(left_start >= begin);
            assert!(center_start >= left_end);
            assert!(right_start >= center_end);
            assert!(end >= right_end);
            assert!(
                (left_start - begin) + (center_start - left_end) + (right_start - center_end)
                    + (end - right_end) + col.width() == column_width
            );
            for _ in begin..left_start {
                write!(
                    output,
                    "{}",
                    Color::new(
                        color::Mode::Normal(color::Base::Reset),
                        color::Mode::Normal(color::Base::Reset)
                    )
                )?;
                write!(&mut output, " ")?;
            }
            self.render_displaystring(&col.left, &mut output)?;
            write!(
                output,
                "{}{}",
                Color::new(
                    color::Mode::Normal(color::Base::Reset),
                    color::Mode::Normal(color::Base::Reset)
                ),
                Style::NORMAL
            )?;
            for _ in left_end..center_start {
                write!(&mut output, " ")?;
            }

            self.render_displaystring(&col.center, &mut output)?;
            write!(
                output,
                "{}{}",
                Color::new(
                    color::Mode::Normal(color::Base::Reset),
                    color::Mode::Normal(color::Base::Reset)
                ),
                Style::NORMAL
            )?;
            for _ in center_end..right_start {
                write!(
                    output,
                    "{}",
                    Color::new(
                        color::Mode::Normal(color::Base::Reset),
                        color::Mode::Normal(color::Base::Reset)
                    )
                )?;
                write!(&mut output, " ")?;
            }

            self.render_displaystring(&col.right, &mut output)?;
            write!(
                output,
                "{}{}",
                Color::new(
                    color::Mode::Normal(color::Base::Reset),
                    color::Mode::Normal(color::Base::Reset)
                ),
                Style::NORMAL
            )?;
            for _ in right_end..end {
                write!(&mut output, " ")?;
            }
            if let Some(left_pos) = col.left.cursor {
                assert!(cursor_position.is_none());
                cursor_position = Some(left_start + left_pos);
            }
            if let Some(center_pos) = col.center.cursor {
                assert!(cursor_position.is_none());
                cursor_position = Some(center_start + center_pos);
            }
            if let Some(right_pos) = col.right.cursor {
                assert!(cursor_position.is_none());
                cursor_position = Some(right_start + right_pos)
            }
        }
        write!(
            output,
            "{}{}",
            Color::new(
                color::Mode::Normal(color::Base::Reset),
                color::Mode::Normal(color::Base::Reset)
            ),
            Style::NORMAL
        )?;
        self.buffer.push(output);
        Ok(cursor_position)
    }

    fn render_displaystring(
        &self,
        string: &DisplayString,
        mut writer: &mut fmt::Write,
    ) -> fmt::Result {
        use std::fmt::Write;
        for component in &string.components {
            write!(
                &mut writer,
                "{}{}{}",
                component.color, component.style, component.text
            )?;
        }
        Ok(())
    }
}
