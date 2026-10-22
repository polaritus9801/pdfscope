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
