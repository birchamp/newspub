//! Snapshot undo history (ARCHITECTURE.md §4).

/// Undo/redo stacks of document snapshots.
#[derive(Clone, Debug)]
pub struct History<T: Clone> {
    undo: Vec<(T, Option<String>)>,
    redo: Vec<T>,
    limit: usize,
}

impl<T: Clone> Default for History<T> {
    fn default() -> Self {
        History { undo: vec![], redo: vec![], limit: 200 }
    }
}

impl<T: Clone> History<T> {
    /// Records `before` as an undo step. If `coalesce` equals the key of the previous
    /// step, the steps merge (the earlier snapshot is kept).
    pub fn record(&mut self, before: T, coalesce: Option<String>) {
        self.redo.clear();
        if let (Some(k), Some((_, Some(prev)))) = (&coalesce, self.undo.last())
            && k == prev
        {
            return;
        }
        self.undo.push((before, coalesce));
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
    }

    /// Breaks coalescing so the next step starts a new undo entry.
    pub fn seal(&mut self) {
        if let Some(last) = self.undo.last_mut() {
            last.1 = None;
        }
    }

    pub fn undo(&mut self, current: T) -> Option<T> {
        let (prev, _) = self.undo.pop()?;
        self.redo.push(current);
        Some(prev)
    }

    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push((current, None));
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }
}
