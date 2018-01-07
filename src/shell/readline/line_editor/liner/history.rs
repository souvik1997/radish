use super::*;

use std::collections::{vec_deque, VecDeque};
use std::iter::IntoIterator;
use std::ops::Index;
use std::ops::IndexMut;
use shell::history::History;

const DEFAULT_MAX_SIZE: usize = 1000;

/// Structure encapsulating command history
pub struct HistoryManager<'a> {
    // TODO: this should eventually be private
    /// Vector of buffers to store history in
    pub buffers: VecDeque<Buffer>,
    pub history_instance: Option<&'a History>,
}

impl<'a> HistoryManager<'a> {
    /// Create new History structure.
    pub fn new(history_instance: Option<&'a History>) -> HistoryManager {
        let buffers = match history_instance {
            Some(h) => {
                let mut buffers = VecDeque::with_capacity(h.len());
                match h.entries() {
                    &Some(ref entries) => for entry in entries {
                        buffers.push_back(Buffer::from(entry.command.as_ref()))
                    },
                    &None => {}
                };
                buffers
            }
            None => VecDeque::with_capacity(0),
        };
        HistoryManager {
            buffers: buffers,
            history_instance: history_instance,
        }
    }

    /// Number of items in history.
    pub fn len(&self) -> usize {
        self.buffers.len()
    }

    /// Go through the history and try to find a buffer which starts the same as the new buffer
    /// given to this function as argument.
    pub fn get_newest_match<'b>(&'a self, curr_position: Option<usize>, new_buff: &'b Buffer) -> Option<&'a Buffer> {
        let pos = curr_position.unwrap_or(self.buffers.len());
        for iter in (0..pos).rev() {
            if let Some(tested) = {
                if iter > self.len() {
                    None
                } else {
                    Some(self.index(iter))
                }
            } {
                if tested.starts_with(new_buff) {
                    return Some(tested);
                }
            }
        }
        None
    }
}

impl<'a> Index<usize> for HistoryManager<'a> {
    type Output = Buffer;

    fn index(&self, index: usize) -> &Buffer {
        &self.buffers[index]
    }
}

impl<'a> IndexMut<usize> for HistoryManager<'a> {
    fn index_mut(&mut self, index: usize) -> &mut Buffer {
        &mut self.buffers[index]
    }
}
