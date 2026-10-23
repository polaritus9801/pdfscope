//! A deliberately rough token estimate, used only to warn the reader before sending.
//!
//! Exact tokenisation belongs to the provider; this is a budget hint, so it stays cheap and
//! predictable: about three characters per token for mixed Chinese/English prose, plus a flat
//! allowance for an image or a page of document text.

/// Roughly how many tokens a request will need.
pub fn estimate_tokens(text_chars: usize, images: usize, pages: usize) -> usize {
    text_chars.div_ceil(3) + images.saturating_mul(850) + pages.saturating_mul(650)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_costs_nothing() {
        assert_eq!(estimate_tokens(0, 0, 0), 0);
    }

    #[test]
    fn text_rounds_up_by_three_characters() {
        assert_eq!(estimate_tokens(1, 0, 0), 1);
        assert_eq!(estimate_tokens(3, 0, 0), 1);
        assert_eq!(estimate_tokens(4, 0, 0), 2);
    }

    #[test]
    fn each_image_and_page_has_a_flat_cost() {
        assert_eq!(estimate_tokens(0, 1, 0), 850);
        assert_eq!(estimate_tokens(0, 0, 1), 650);
        assert_eq!(estimate_tokens(0, 2, 3), 1700 + 1950);
    }

    #[test]
    fn the_parts_add_up() {
        assert_eq!(estimate_tokens(30, 1, 2), 10 + 850 + 1300);
    }
}
