//! A bounded undo/redo stack.
//!
//! One rule matters here: a new edit truncates the redo branch. Keeping that in a tested type is
//! cheaper than keeping it in three closures.

/// Past and future states, with a cap on how far back undo can reach.
#[derive(Debug, Clone)]
pub struct History<T> {
    past: Vec<T>,
    future: Vec<T>,
    limit: usize,
}

impl<T: Clone> History<T> {
    /// A stack that keeps at most `limit` past states.
    pub fn new(limit: usize) -> Self {
        Self {
            past: Vec::new(),
            future: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Records the state before an edit, and drops the redo branch.
    pub fn push(&mut self, state: T) {
        self.past.push(state);
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
        self.future.clear();
    }

    /// Takes the previous state, moving `current` onto the redo branch.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let previous = self.past.pop()?;
        self.future.push(current);
        Some(previous)
    }

    /// Takes the next state, moving `current` onto the undo branch.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.future.pop()?;
        self.past.push(current);
        if self.past.len() > self.limit {
            self.past.remove(0);
        }
        Some(next)
    }

    /// Whether there is anything to undo.
    pub fn can_undo(&self) -> bool {
        !self.past.is_empty()
    }

    /// Whether there is anything to redo.
    pub fn can_redo(&self) -> bool {
        !self.future.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_returns_the_previous_state() {
        let mut history = History::new(10);
        history.push("a");
        assert_eq!(history.undo("b"), Some("a"));
        assert_eq!(history.undo("a"), None);
    }

    #[test]
    fn redo_returns_the_state_that_was_undone() {
        let mut history = History::new(10);
        history.push("a");
        let restored = history.undo("b").unwrap();
        assert_eq!(restored, "a");
        assert_eq!(history.redo("a"), Some("b"));
        assert_eq!(history.redo("b"), None);
    }

    #[test]
    fn a_new_edit_clears_the_redo_branch() {
        let mut history = History::new(10);
        history.push("a");
        let _ = history.undo("b");
        assert!(history.can_redo());
        history.push("a");
        assert!(!history.can_redo());
    }

    #[test]
    fn the_limit_drops_the_oldest_state() {
        let mut history = History::new(2);
        history.push(1);
        history.push(2);
        history.push(3);
        assert_eq!(history.undo(4), Some(3));
        assert_eq!(history.undo(3), Some(2));
        assert_eq!(history.undo(2), None);
    }

    #[test]
    fn undo_and_redo_walk_the_same_states() {
        let mut history = History::new(10);
        history.push("one");
        history.push("two");

        assert_eq!(history.undo("three"), Some("two"));
        assert_eq!(history.undo("two"), Some("one"));
        assert_eq!(history.redo("one"), Some("two"));
        assert_eq!(history.redo("two"), Some("three"));
    }

    #[test]
    fn an_empty_history_reports_nothing_to_do() {
        let mut history: History<u8> = History::new(5);
        assert!(!history.can_undo());
        assert!(!history.can_redo());
        assert_eq!(history.undo(1), None);
        assert_eq!(history.redo(1), None);
    }
}
