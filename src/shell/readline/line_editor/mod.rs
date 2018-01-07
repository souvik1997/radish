use super::display::*;
use super::termion;
use super::super::completion::Completer;
use super::super::history::History;
use self::unicode_width::UnicodeWidthStr;

mod liner;
extern crate unicode_segmentation;
use unicode_segmentation::UnicodeSegmentation;
use self::liner::KeyMap;
use super::ReadlineEvent;

// Single row, single column
pub struct LineEditor<'a, 'b: 'a> {
    prompt: DisplayString<'a>,
    editor: liner::emacs::Emacs<'a, 'b>,
}

impl<'a, 'b: 'a> LineEditor<'a, 'b> {
    pub fn new(prompt: DisplayString<'a>, completer: &'a mut Completer<'b>, history: &'a History) -> LineEditor<'a, 'b> {
        LineEditor {
            prompt: prompt,
            editor: liner::emacs::Emacs::new(liner::Editor::new(Some(history), Some(completer))),
        }
    }

    pub fn handle_input(&mut self, key: termion::event::Key) -> ReadlineEvent {
        self.editor.handle_key(key, &mut |_| {})
    }

    pub fn buffer(&self) -> &[char] {
        &self.editor.editor().current_buffer().data
    }

    pub fn cursor(&self) -> usize {
        self.editor.editor().cursor()
    }
}

impl<'a, 'b: 'a> Render for LineEditor<'a, 'b> {
    fn render<F: FnMut(&Row)>(&mut self, render_fn: &mut F, width: usize) {
        // https://users.rust-lang.org/t/solved-how-to-split-string-into-multiple-sub-strings-with-given-length/10542/9
        fn sub_strings(string: &str, sub_len: usize) -> Vec<&str> {
            let mut subs = Vec::with_capacity(string.len() / sub_len);
            let mut iter = string.chars();
            let mut pos = 0;

            while pos < string.len() {
                let mut len = 0;
                for ch in iter.by_ref().take(sub_len) {
                    len += ch.len_utf8();
                }
                subs.push(&string[pos..pos + len]);
                pos += len;
            }
            subs
        }
        let buffer_string = self.buffer().iter().cloned().collect::<String>();
        let (cursor_split_left, _) = buffer_string.split_at(self.cursor());
        let graphemes: Vec<&str> = cursor_split_left.graphemes(true).collect();
        let cursor_position = graphemes.len() + self.prompt.width();

        // word wrap line
        let mut lines: Vec<Row> = vec![
            Row {
                columns: vec![
                    Column {
                        left: self.prompt.clone(),
                        center: DisplayString::new(),
                        right: DisplayString::new(),
                    },
                ],
            },
        ];
        if self.prompt.width() >= cursor_position {
            lines[0].columns[0].left.cursor = Some(cursor_position);
        }
        let mut total_prev_length = 0;
        for (index, word) in buffer_string.split_word_bounds().enumerate() {
            for word in sub_strings(word, width) {
                let total_length = lines.last().unwrap().width();
                let total_line_length = total_prev_length + total_length;
                let component;
                if index == 0 {
                    component = DisplayStringComponent::new(
                        word,
                        Color::new(
                            color::Mode::Normal(color::Base::White),
                            color::Mode::Normal(color::Base::Reset),
                        ),
                        Style::BOLD,
                    );
                } else {
                    component = DisplayStringComponent::new(
                        word,
                        Color::new(
                            color::Mode::Normal(color::Base::Blue),
                            color::Mode::Light(color::Base::Reset),
                        ),
                        Style::NORMAL,
                    );
                }
                if total_length + component.width() >= width {
                    let component_cursor;
                    if cursor_position > total_line_length && total_line_length + component.width() >= cursor_position {
                        component_cursor = Some(cursor_position - total_line_length);
                    } else {
                        component_cursor = None;
                    }
                    total_prev_length += total_length;
                    lines.push(Row {
                        columns: vec![
                            Column {
                                left: DisplayString {
                                    components: vec![component],
                                    cursor: component_cursor,
                                },
                                center: DisplayString::new(),
                                right: DisplayString::new(),
                            },
                        ],
                    });
                } else {
                    let last_index = lines.len() - 1;
                    let row = &mut lines[last_index];
                    if cursor_position > total_line_length && total_line_length + component.width() >= cursor_position {
                        assert!(row.columns[0].left.cursor.is_none());
                        row.columns[0].left.cursor = Some((cursor_position - total_line_length) + total_length);
                    }
                    row.columns[0].left.components.push(component);
                }
            }
        }
        for row in &lines {
            render_fn(&row);
        }
    }
}
