//! One icon language for the interface.
//!
//! Every interface icon is drawn on the same 24 grid with a 1.6 stroke and round caps. Provider
//! marks are the only filled, coloured graphics; anything unknown gets a monogram tile.

use leptos::prelude::*;

/// An outline icon from the shared set.
pub(crate) fn icon(name: &'static str, size: u32) -> impl IntoView {
    let body = match name {
        "plus" => r#"<path d="M12 5v14M5 12h14"/>"#,
        "minus" => r#"<path d="M5 12h14"/>"#,
        "chevron-down" => r#"<path d="M6 9l6 6 6-6"/>"#,
        "chevron-left" => r#"<path d="M15 6l-6 6 6 6"/>"#,
        "chevron-right" => r#"<path d="M9 6l6 6-6 6"/>"#,
        "history" => {
            r#"<path d="M3 12a9 9 0 1 0 2.7-6.4"/><path d="M3 4v5h5"/><path d="M12 8v4l3 2"/>"#
        }
        "attach" => {
            r#"<path d="M20 11.5l-7.8 7.8a5 5 0 0 1-7.1-7.1l8.2-8.2a3.4 3.4 0 0 1 4.8 4.8l-8.2 8.2a1.8 1.8 0 0 1-2.5-2.5l7.4-7.4"/>"#
        }
        "globe" => {
            r#"<circle cx="12" cy="12" r="8.5"/><path d="M3.5 12h17M12 3.5c2.4 2.3 3.6 5.1 3.6 8.5s-1.2 6.2-3.6 8.5c-2.4-2.3-3.6-5.1-3.6-8.5S9.6 5.8 12 3.5z"/>"#
        }
        "at" => {
            r#"<circle cx="12" cy="12" r="4"/><path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8"/>"#
        }
        "send" => r#"<path d="M12 19V5M6 11l6-6 6 6"/>"#,
        "stop" => r#"<rect x="7" y="7" width="10" height="10" rx="2"/>"#,
        "copy" => {
            r#"<rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V5a2 2 0 0 1 2-2h8"/>"#
        }
        "retry" => r#"<path d="M20 12a8 8 0 1 1-2.3-5.7"/><path d="M20 4v5h-5"/>"#,
        "edit" => r#"<path d="M4 20h4l10-10-4-4L4 16v4z"/><path d="M13.5 5.5l5 5"/>"#,
        "branch" => {
            r#"<circle cx="6" cy="5" r="2"/><circle cx="6" cy="19" r="2"/><circle cx="18" cy="12" r="2"/><path d="M6 7v10M8 5h4a4 4 0 0 1 4 4v1"/>"#
        }
        "quote" => r#"<path d="M9 7H5v6h4l-2 4"/><path d="M19 7h-4v6h4l-2 4"/>"#,
        "trash" => r#"<path d="M4 7h16M9 7V4h6v3M7 7l1 13h8l1-13"/>"#,
        "pin" => r#"<path d="M9 4h6l-1 6 4 3H6l4-3-1-6z"/><path d="M12 13v7"/>"#,
        "search" => r#"<circle cx="11" cy="11" r="6.5"/><path d="M16 16l4 4"/>"#,
        "close" => r#"<path d="M6 6l12 12M18 6L6 18"/>"#,
        "check" => r#"<path d="M5 12l5 5 9-10"/>"#,
        "alert" => r#"<path d="M12 3l9 17H3z"/><path d="M12 9v5M12 17h.01"/>"#,
        "page" => r#"<path d="M7 3h7l4 4v14H7z"/><path d="M14 3v4h4"/>"#,
        "pages" => {
            r#"<rect x="7" y="3.5" width="10" height="14" rx="1.5"/><path d="M10 20.5h7a2 2 0 0 0 2-2V8"/>"#
        }
        "image" => {
            r#"<rect x="3.5" y="4.5" width="17" height="15" rx="2"/><circle cx="9" cy="10" r="1.5"/><path d="M4 17l5-5 4 4 3-3 4 4"/>"#
        }
        "file" => r#"<path d="M6 3h8l4 4v14H6z"/><path d="M14 3v4h4"/><path d="M9 12h6M9 16h6"/>"#,
        "link" => {
            r#"<path d="M10 14a4 4 0 0 1 0-6l2-2a4 4 0 0 1 6 6l-1 1"/><path d="M14 10a4 4 0 0 1 0 6l-2 2a4 4 0 0 1-6-6l1-1"/>"#
        }
        "sparkle" => r#"<path d="M12 3l2 6 6 2-6 2-2 6-2-6-6-2 6-2z"/>"#,
        "brain" => {
            r#"<path d="M12 5a3 3 0 0 0-6 0v1a3 3 0 0 0-2 5 3 3 0 0 0 2 5 3 3 0 0 0 6 2z"/><path d="M12 5a3 3 0 0 1 6 0v1a3 3 0 0 1 2 5 3 3 0 0 1-2 5 3 3 0 0 1-6 2z"/><path d="M12 5v15"/>"#
        }
        "settings" => {
            r#"<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>"#
        }
        "book" => {
            r#"<path d="M12 6c-2-1.5-4.5-2-8-1.5V19c3.5-.5 6 0 8 1.5 2-1.5 4.5-2 8-1.5V4.5C16.5 4 14 4.5 12 6z"/><path d="M12 6v14.5"/>"#
        }
        _ => r#"<circle cx="12" cy="12" r="8"/>"#,
    };

    view! {
        <svg
            viewBox="0 0 24 24"
            width=size
            height=size
            fill="none"
            stroke="currentColor"
            stroke-width="1.6"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
            focusable="false"
            inner_html=body
        ></svg>
    }
}

/// The colour used for a provider's mark and monogram.
fn provider_colour(preset: &str) -> &'static str {
    let lowered = preset.to_lowercase();
    if lowered.contains("deepseek") {
        "#4D6BFE"
    } else if lowered.contains("openai") {
        "#10A37F"
    } else if lowered.contains("anthropic") || lowered.contains("claude") {
        "#D97757"
    } else if lowered.contains("gemini") || lowered.contains("google") {
        "#4285F4"
    } else if lowered.contains("mimo") || lowered.contains("moonshot") {
        "#7F5CBF"
    } else if lowered.contains("openrouter") {
        "#6B7280"
    } else if lowered.contains("ollama") {
        "#111827"
    } else {
        "#7F5CBF"
    }
}

/// The official brand SVG shipped in `public/brands`, if this provider has one.
fn brand_slug(preset: &str) -> Option<&'static str> {
    let lowered = preset.to_lowercase();
    if lowered.contains("deepseek") {
        Some("deepseek")
    } else if lowered.contains("openai") {
        Some("openai")
    } else if lowered.contains("anthropic") || lowered.contains("claude") {
        Some("anthropic")
    } else if lowered.contains("gemini") || lowered.contains("google") {
        Some("gemini")
    } else if lowered.contains("mimo") {
        Some("xiaomi")
    } else if lowered.contains("moonshot") {
        Some("moonshot")
    } else if lowered.contains("openrouter") {
        Some("openrouter")
    } else if lowered.contains("ollama") {
        Some("ollama")
    } else {
        None
    }
}

/// An official brand mark as a square SVG; unknown providers fall back to a monogram tile.
pub(crate) fn provider_mark(preset: &str, size: u32) -> AnyView {
    if let Some(slug) = brand_slug(preset) {
        return view! {
            <img
                class="provider-mark"
                src=format!("/brands/{slug}.svg")
                width=size
                height=size
                alt=""
                draggable="false"
            />
        }
        .into_any();
    }

    let colour = provider_colour(preset);
    let letter = preset
        .chars()
        .next()
        .unwrap_or('A')
        .to_uppercase()
        .to_string();
    view! {
        <span class="provider-letter" style=format!("background:{colour}")>{letter}</span>
    }
    .into_any()
}

/// A rounded tile holding the provider mark.
pub(crate) fn provider_tile(preset: String, size: u32) -> impl IntoView {
    let mark = provider_mark(&preset, (size as f64 * 0.64) as u32);
    view! {
        <span
            class="provider-tile"
            style=format!("width:{size}px;height:{size}px")
            title=preset
        >
            {mark}
        </span>
    }
}

/// The attachment paperclip from the product's 16 px icon set.
pub(crate) fn paperclip() -> impl IntoView {
    view! {
        <svg
            width="16"
            height="16"
            viewBox="0 0 16 16"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
            focusable="false"
        >
            <path
                d="M5.5498 9.75V5H6.9502V9.75C6.9502 10.3299 7.4201 10.7998 8 10.7998C8.5799 10.7998 9.0498 10.3299 9.0498 9.75V4.5C9.0498 2.9536 7.7964 1.7002 6.25 1.7002C4.7036 1.7002 3.4502 2.9536 3.4502 4.5V9.75C3.4502 12.2629 5.4871 14.2998 8 14.2998C10.5129 14.2998 12.5498 12.2629 12.5498 9.75V4H13.9502V9.75C13.9502 13.0361 11.2861 15.7002 8 15.7002C4.71391 15.7002 2.0498 13.0361 2.0498 9.75V4.5C2.04981 2.1804 3.9304 0.299806 6.25 0.299805C8.5696 0.299805 10.4502 2.1804 10.4502 4.5V9.75C10.4502 11.1031 9.3531 12.2002 8 12.2002C6.6469 12.2002 5.5498 11.1031 5.5498 9.75Z"
                fill="currentColor"
            ></path>
        </svg>
    }
}
