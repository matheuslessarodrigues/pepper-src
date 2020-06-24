use std::ops::{Index, IndexMut};

use crate::{
    buffer::{Buffer, BufferCollection, BufferHandle, TextRef},
    buffer_position::{BufferOffset, BufferRange},
    cursor::CursorCollection,
    history::EditKind,
};

pub struct BufferView {
    pub buffer_handle: BufferHandle,
    pub cursors: CursorCollection,
}

impl BufferView {
    pub fn with_handle(buffer_handle: BufferHandle) -> Self {
        Self {
            buffer_handle,
            cursors: CursorCollection::new(),
        }
    }

    pub fn buffer_mut<'a>(&mut self, buffers: &'a mut BufferCollection) -> &'a mut Buffer {
        &mut buffers[self.buffer_handle]
    }

    pub fn move_cursors(&mut self, buffers: &BufferCollection, offset: BufferOffset) {
        let buffer = &buffers[self.buffer_handle];
        self.cursors.change_all(|cursor| {
            let mut target = BufferOffset::from(cursor.position) + offset;

            target.line_offset = target
                .line_offset
                .min(buffer.content.line_count() as isize - 1)
                .max(0);
            let target_line_len = buffer.content.line(target.line_offset as _).char_count();
            target.column_offset = target.column_offset.min(target_line_len as _);

            cursor.position = target.into();
        });
    }

    pub fn collapse_cursor_anchors(&mut self) {
        self.cursors.change_all(|cursor| {
            cursor.anchor = cursor.position;
        });
    }

    pub fn swap_cursor_position_and_anchor(&mut self) {
        self.cursors.change_all(|cursor| {
            std::mem::swap(&mut cursor.position, &mut cursor.anchor);
        });
    }

    fn commit_edits(&mut self, buffers: &mut BufferCollection) {
        buffers[self.buffer_handle].history.commit_edits();
    }

    fn insert_text(
        &mut self,
        buffers: &mut BufferCollection,
        text: TextRef,
        ranges: &mut Vec<BufferRange>,
    ) {
        let buffer = &mut buffers[self.buffer_handle];
        ranges.clear();
        self.cursors.change_all(|cursor| {
            let range = buffer.insert_text(cursor.position, text);
            cursor.insert(range);
            ranges.push(range);
        });
    }

    fn remove_in_selection(&mut self, buffers: &mut BufferCollection, ranges: &mut Vec<BufferRange>) {
        let buffer = &mut buffers[self.buffer_handle];
        ranges.clear();
        self.cursors.change_all(|cursor| {
            let range = cursor.range();
            buffer.remove_range(range);
            cursor.remove(range);
            ranges.push(range);
        });
    }
}

#[derive(Default)]
pub struct BufferViewCollection {
    buffer_views: Vec<BufferView>,
    temp_ranges: Vec<BufferRange>,
}

impl BufferViewCollection {
    pub fn len(&self) -> usize {
        self.buffer_views.len()
    }

    pub fn push(&mut self, buffer_view: BufferView) {
        self.buffer_views.push(buffer_view);
    }

    fn current_and_other_buffer_views_mut<'a>(
        &'a mut self,
        index: usize,
    ) -> (
        &'a mut BufferView,
        impl Iterator<Item = &'a mut BufferView>,
        &mut Vec<BufferRange>,
    ) {
        let (before, after) = self.buffer_views.split_at_mut(index);
        let (current, after) = after.split_at_mut(1);
        let current = &mut current[0];
        let current_buffer_handle = current.buffer_handle;

        let iter = before
            .iter_mut()
            .filter(move |v| v.buffer_handle == current_buffer_handle)
            .chain(
                after
                    .iter_mut()
                    .filter(move |v| v.buffer_handle == current_buffer_handle),
            );

        (current, iter, &mut self.temp_ranges)
    }

    pub fn move_cursors(
        &mut self,
        buffers: &mut BufferCollection,
        index: Option<usize>,
        offset: BufferOffset,
    ) {
        if let Some(index) = index {
            self.buffer_views[index].move_cursors(buffers, offset);
        }
    }

    pub fn collapse_cursor_anchors(&mut self, index: Option<usize>) {
        if let Some(index) = index {
            self.buffer_views[index].collapse_cursor_anchors();
        }
    }

    pub fn swap_cursor_position_and_anchor(&mut self, index: Option<usize>) {
        if let Some(index) = index {
            self.buffer_views[index].swap_cursor_position_and_anchor();
        }
    }

    pub fn commit_edits(&mut self, buffers: &mut BufferCollection, index: Option<usize>) {
        if let Some(index) = index {
            self.buffer_views[index].commit_edits(buffers);
        }
    }

    pub fn insert_text(
        &mut self,
        buffers: &mut BufferCollection,
        index: Option<usize>,
        text: TextRef,
    ) {
        let index = match index {
            Some(index) => index,
            None => return,
        };

        let (current_view, other_views, temp_ranges) =
            self.current_and_other_buffer_views_mut(index);
        current_view.insert_text(buffers, text, temp_ranges);

        for view in other_views {
            view.cursors.change_all(|cursor| {
                for range in temp_ranges.iter() {
                    cursor.insert(*range);
                }
            });
        }
    }

    pub fn remove_in_selection(&mut self, buffers: &mut BufferCollection, index: Option<usize>) {
        let index = match index {
            Some(index) => index,
            None => return,
        };

        let (current_view, other_views, temp_ranges) =
            self.current_and_other_buffer_views_mut(index);
        current_view.remove_in_selection(buffers, temp_ranges);
        for view in other_views {
            view.cursors.change_all(|cursor| {
                for range in temp_ranges.iter() {
                    cursor.remove(*range);
                }
            });
        }
    }

    pub fn undo(&mut self, buffers: &mut BufferCollection, index: Option<usize>) {
        let index = match index {
            Some(index) => index,
            None => return,
        };

        let buffer = &mut buffers[self.buffer_views[index].buffer_handle];
        self.apply_edits(index, buffer.undo());
    }

    pub fn redo(&mut self, buffers: &mut BufferCollection, index: Option<usize>) {
        let index = match index {
            Some(index) => index,
            None => return,
        };

        let buffer = &mut buffers[self.buffer_views[index].buffer_handle];
        self.apply_edits(index, buffer.redo());
    }

    fn apply_edits(&mut self, index: usize, edits: impl Iterator<Item = (EditKind, BufferRange)>) {
        for (kind, range) in edits {
            let (current_view, other_views, _) = self.current_and_other_buffer_views_mut(index);
            match kind {
                EditKind::Insert => {
                    current_view.cursors.change_all(|c| {
                        c.position = range.to;
                        c.anchor = range.to;
                    });
                    for view in other_views {
                        view.cursors.change_all(|c| c.insert(range));
                    }
                }
                EditKind::Remove => {
                    current_view.cursors.change_all(|c| {
                        c.position = range.from;
                        c.anchor = range.from;
                    });
                    for view in other_views {
                        view.cursors.change_all(|c| c.remove(range));
                    }
                }
            }
        }
    }
}

impl Index<usize> for BufferViewCollection {
    type Output = BufferView;
    fn index(&self, index: usize) -> &BufferView {
        &self.buffer_views[index]
    }
}

impl IndexMut<usize> for BufferViewCollection {
    fn index_mut(&mut self, index: usize) -> &mut BufferView {
        &mut self.buffer_views[index]
    }
}
