//! A set of page numbers, kept sorted and deduplicated.
//!
//! The page bars, the manager dialog and drag selection all mutate the same set; the rules for
//! merging overlapping ranges and splitting ranges on subtraction live here so they can be tested
//! without a browser.

/// Adds one page if it is missing.
pub fn toggle_page(pages: &mut Vec<u32>, page: u32) {
    if page == 0 {
        return;
    }
    match pages.binary_search(&page) {
        Ok(index) => {
            pages.remove(index);
        }
        Err(index) => pages.insert(index, page),
    }
}

/// Adds every page in `from..=to`, in either direction; duplicates collapse.
pub fn add_range(pages: &mut Vec<u32>, from: u32, to: u32) {
    let (low, high) = if from <= to { (from, to) } else { (to, from) };
    let low = low.max(1);
    if high == 0 || low > high {
        return;
    }
    for page in low..=high {
        if let Err(index) = pages.binary_search(&page) {
            pages.insert(index, page);
        }
    }
}

/// Removes every page in `from..=to`, in either direction.
pub fn subtract_range(pages: &mut Vec<u32>, from: u32, to: u32) {
    let (low, high) = if from <= to { (from, to) } else { (to, from) };
    let low = low.max(1);
    pages.retain(|page| *page < low || *page > high);
}

/// Removes one page, if present.
pub fn remove_page(pages: &mut Vec<u32>, page: u32) {
    pages.retain(|entry| *entry != page);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(entries: &[u32]) -> Vec<u32> {
        entries.to_vec()
    }

    #[test]
    fn toggling_adds_then_removes_a_page_in_order() {
        let mut pages = set(&[3, 9]);
        toggle_page(&mut pages, 6);
        assert_eq!(pages, set(&[3, 6, 9]));
        toggle_page(&mut pages, 6);
        assert_eq!(pages, set(&[3, 9]));
    }

    #[test]
    fn adding_a_range_merges_with_existing_pages() {
        let mut pages = set(&[1, 2, 3, 9, 10, 11]);
        add_range(&mut pages, 6, 7);
        assert_eq!(pages, set(&[1, 2, 3, 6, 7, 9, 10, 11]));
    }

    #[test]
    fn adding_a_reversed_range_is_the_same_as_forwards() {
        let mut forward = Vec::new();
        add_range(&mut forward, 4, 7);
        let mut backward = Vec::new();
        add_range(&mut backward, 7, 4);
        assert_eq!(forward, backward);
    }

    #[test]
    fn nested_and_duplicate_ranges_collapse() {
        let mut pages = Vec::new();
        add_range(&mut pages, 2, 8);
        add_range(&mut pages, 3, 5);
        add_range(&mut pages, 2, 8);
        assert_eq!(pages, set(&[2, 3, 4, 5, 6, 7, 8]));
    }

    #[test]
    fn subtracting_a_middle_range_splits_the_set() {
        let mut pages: Vec<u32> = (1..=10).collect();
        subtract_range(&mut pages, 4, 6);
        assert_eq!(pages, set(&[1, 2, 3, 7, 8, 9, 10]));
    }

    #[test]
    fn subtracting_a_reversed_range_works() {
        let mut pages: Vec<u32> = (1..=10).collect();
        subtract_range(&mut pages, 6, 4);
        assert_eq!(pages, set(&[1, 2, 3, 7, 8, 9, 10]));
    }

    #[test]
    fn zero_is_never_a_page() {
        let mut pages = Vec::new();
        add_range(&mut pages, 0, 3);
        toggle_page(&mut pages, 0);
        assert_eq!(pages, set(&[1, 2, 3]));
    }

    #[test]
    fn the_set_stays_sorted_and_unique_under_mixed_edits() {
        let mut pages = Vec::new();
        add_range(&mut pages, 5, 9);
        add_range(&mut pages, 1, 4);
        toggle_page(&mut pages, 7);
        toggle_page(&mut pages, 2);
        subtract_range(&mut pages, 8, 9);
        assert_eq!(pages, set(&[1, 3, 4, 5, 6]));
        assert!(pages.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn removing_a_page_that_is_not_there_changes_nothing() {
        let mut pages = set(&[1, 2]);
        remove_page(&mut pages, 9);
        assert_eq!(pages, set(&[1, 2]));
    }
}

/// How much of the document a question may see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Every page.
    Document,
    /// The page in view.
    Current,
    /// Only the passages and regions the reader added.
    References,
    /// An explicit set of pages.
    Selected,
}

impl Scope {
    /// Maps the interface's byte: 0 document, 1 current, 2 references, anything else selected.
    pub fn from_byte(mode: u8) -> Self {
        match mode {
            0 => Self::Document,
            1 => Self::Current,
            2 => Self::References,
            _ => Self::Selected,
        }
    }
}

/// The pages a request should carry, and how many were wanted before the cap.
///
/// Returns `(pages, wanted)`: `wanted` is larger than `pages.len()` when the cap cut the selection,
/// which is what the composer reports to the reader instead of truncating silently.
pub fn request_pages(
    scope: Scope,
    total: u32,
    current: u32,
    selected: &[u32],
    cap: usize,
) -> (Vec<u32>, usize) {
    if total == 0 {
        return (Vec::new(), 0);
    }

    let wanted: Vec<u32> = match scope {
        Scope::Document => (1..=total).collect(),
        Scope::Current => vec![current.clamp(1, total)],
        Scope::References => Vec::new(),
        Scope::Selected => {
            let mut kept: Vec<u32> = selected
                .iter()
                .copied()
                .filter(|page| (1..=total).contains(page))
                .collect();
            kept.sort_unstable();
            kept.dedup();
            kept
        }
    };

    let wanted_count = wanted.len();
    (wanted.into_iter().take(cap).collect(), wanted_count)
}

#[cfg(test)]
mod request_tests {
    use super::*;

    #[test]
    fn the_whole_document_is_capped_and_reported() {
        let (pages, wanted) = request_pages(Scope::Document, 100, 7, &[], 20);
        assert_eq!(pages.len(), 20);
        assert_eq!(pages.first(), Some(&1));
        assert_eq!(wanted, 100);
    }

    #[test]
    fn the_current_page_ignores_the_cap() {
        let (pages, wanted) = request_pages(Scope::Current, 300, 42, &[], 20);
        assert_eq!(pages, vec![42]);
        assert_eq!(wanted, 1);
    }

    #[test]
    fn the_current_page_is_clamped_to_the_document() {
        assert_eq!(request_pages(Scope::Current, 10, 99, &[], 20).0, vec![10]);
        assert_eq!(request_pages(Scope::Current, 10, 0, &[], 20).0, vec![1]);
    }

    #[test]
    fn references_only_sends_nothing() {
        let (pages, wanted) = request_pages(Scope::References, 50, 3, &[2, 4], 20);
        assert!(pages.is_empty());
        assert_eq!(wanted, 0);
    }

    #[test]
    fn the_selected_set_is_sorted_deduplicated_and_filtered() {
        let (pages, wanted) = request_pages(Scope::Selected, 10, 1, &[5, 2, 5, 11, 0], 20);
        assert_eq!(pages, vec![2, 5]);
        assert_eq!(wanted, 2);
    }

    #[test]
    fn a_large_selection_is_capped_and_reported() {
        let selected: Vec<u32> = (1..=50).collect();
        let (pages, wanted) = request_pages(Scope::Selected, 100, 1, &selected, 20);
        assert_eq!(pages.len(), 20);
        assert_eq!(wanted, 50);
    }

    #[test]
    fn an_empty_document_sends_nothing_in_every_scope() {
        for scope in [
            Scope::Document,
            Scope::Current,
            Scope::References,
            Scope::Selected,
        ] {
            let (pages, wanted) = request_pages(scope, 0, 1, &[1], 20);
            assert!(pages.is_empty());
            assert_eq!(wanted, 0);
        }
    }

    #[test]
    fn the_byte_mapping_is_stable() {
        assert_eq!(Scope::from_byte(0), Scope::Document);
        assert_eq!(Scope::from_byte(1), Scope::Current);
        assert_eq!(Scope::from_byte(2), Scope::References);
        assert_eq!(Scope::from_byte(3), Scope::Selected);
        assert_eq!(Scope::from_byte(99), Scope::Selected);
    }
}
