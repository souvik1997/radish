use super::Engine;
use std::fs;
use std::collections::BTreeSet;
use std::borrow::Cow;
use std::path::{self, Path};

// from https://github.com/kkawakam/rustyline/blob/master/src/completion.rs, MIT license

impl Engine for PathCompletion {
    fn completions<'a>(&'a mut self, start: &str, line: &str) -> Option<Vec<(Cow<'a, str>, Cow<'a, str>)>> {
        let path = unescape(start, ESCAPE_CHAR);
        let matches = filename_complete(&path, ESCAPE_CHAR, &self.break_chars);
        if matches.is_empty() {
            None
        } else {
            Some(matches)
        }
    }

    fn category<'a>(&'a self) -> &'a str {
        "Files"
    }
}

pub struct PathCompletion {
    break_chars: BTreeSet<char>,
}

static DEFAULT_BREAK_CHARS: [char; 18] = [' ', '\t', '\n', '"', '\\', '\'', '`', '@', '$', '>',
                                          '<', '=', ';', '|', '&', '{', '(', '\0'];
static ESCAPE_CHAR: Option<char> = Some('\\');

impl PathCompletion {
    pub fn new() -> PathCompletion {
        PathCompletion { break_chars: DEFAULT_BREAK_CHARS.iter().cloned().collect() }
    }
}

/// Remove escape char
pub fn unescape(input: &str, esc_char: Option<char>) -> Cow<str> {
    if esc_char.is_none() {
        return Cow::Borrowed(input);
    }
    let esc_char = esc_char.unwrap();
    let n = input.chars().filter(|&c| c == esc_char).count();
    if n == 0 {
        return Cow::Borrowed(input);
    }
    let mut result = String::with_capacity(input.len() - n);
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == esc_char {
            if let Some(ch) = chars.next() {
                result.push(ch);
            }
        } else {
            result.push(ch);
        }
    }
    Cow::Owned(result)
}

/// Escape any `break_chars` in `input` string with `esc_char`.
/// For example, '/User Information' becomes '/User\ Information'
/// when space is a breaking char and '\' the escape char.
pub fn escape(input: String, esc_char: Option<char>, break_chars: &BTreeSet<char>) -> String {
    if esc_char.is_none() {
        return input;
    }
    let esc_char = esc_char.unwrap();
    let n = input
        .chars()
        .filter(|c| break_chars.contains(c))
        .count();
    if n == 0 {
        return input;
    }
    let mut result = String::with_capacity(input.len() + n);

    for c in input.chars() {
        if break_chars.contains(&c) {
            result.push(esc_char);
        }
        result.push(c);
    }
    result
}

fn filename_complete(path: &str,
                     esc_char: Option<char>,
                     break_chars: &BTreeSet<char>)
                     -> Vec<(Cow<'static, str>, Cow<'static, str>)> {
    use std::env::{current_dir, home_dir};

    let sep = path::MAIN_SEPARATOR;
    let (dir_name, file_name) = match path.rfind(sep) {
        Some(idx) => path.split_at(idx + sep.len_utf8()),
        None => ("", path),
    };

    let dir_path = Path::new(dir_name);
    let dir = if dir_path.starts_with("~") {
        // ~[/...]
        if let Some(home) = home_dir() {
            match dir_path.strip_prefix("~") {
                Ok(rel_path) => home.join(rel_path),
                _ => home,
            }
        } else {
            dir_path.to_path_buf()
        }
    } else if dir_path.is_relative() {
        // TODO ~user[/...] (https://crates.io/crates/users)
        if let Ok(cwd) = current_dir() {
            cwd.join(dir_path)
        } else {
            dir_path.to_path_buf()
        }
    } else {
        dir_path.to_path_buf()
    };

    let mut entries: Vec<(Cow<str>, Cow<str>)> = Vec::new();
    if let Ok(read_dir) = dir.read_dir() {
        for entry in read_dir {
            if let Ok(entry) = entry {
                if let Some(s) = entry.file_name().to_str() {
                    if s.starts_with(file_name) {
                        let mut path = String::from(dir_name) + s;
                        let metadata = fs::symlink_metadata(entry.path());
                        if let Ok(metadata) = metadata {
                            if metadata.is_dir() {
                                path.push(sep);
                                entries.push((Cow::Owned(escape(path, esc_char, break_chars)), Cow::Borrowed("Directory")));
                            } else if metadata.is_file() {
                                entries.push((Cow::Owned(escape(path, esc_char, break_chars)), Cow::Borrowed("File")));
                            } else {
                                entries.push((Cow::Owned(escape(path, esc_char, break_chars)), Cow::Borrowed("Unknown")));
                            }
                        } else {
                            entries.push((Cow::Owned(escape(path, esc_char, break_chars)), Cow::Borrowed("Unknown")));
                        }
                    }
                }
            }
        }
    }

    entries
}
