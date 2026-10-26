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

/// The compact PDF mark used by OOBE and the recent/recommended shelves.
pub(crate) fn pdf_mark(size: u32) -> impl IntoView {
    view! {
        <img
            src="/file-icons/file-type/pdf.svg"
            width=size
            height=size
            alt=""
            draggable="false"
        />
    }
}

/// The previous red PDF mark, retained temporarily for rollback.
#[allow(dead_code)]
pub(crate) fn pdf_file(size: u32) -> impl IntoView {
    view! {
        <svg
            width=size
            height=size
            viewBox="0 0 1024 1024"
            fill="none"
            xmlns="http://www.w3.org/2000/svg"
            aria-hidden="true"
            focusable="false"
        >
            <path
                d="M658.68 63.95H269.36c-78.06 0-141.33 63.28-141.33 141.33v612.95c0 78.06 63.28 141.33 141.33 141.33h485.49c78.06 0 141.33-63.28 141.33-141.33V301.44c0-31.83-12.64-62.35-35.15-84.85l-117.5-117.5a120.008 120.008 0 0 0-84.85-35.14z"
                fill="#FF7878"
            ></path>
            <path
                d="M861.03 216.59l-117.5-117.5a120.001 120.001 0 0 0-61.75-32.9v131.88c0 41.87 33.94 75.81 75.81 75.81h135.37a120.058 120.058 0 0 0-31.93-57.29zM649.24 554.59c-22.79 0-45.38 3.31-68.16 7.6-26.87-24.76-49.66-54-66.99-86.56 18.5-60.63 19.47-101.76 5.45-121.26-6.43-8.58-17.14-14.04-28.04-14.04-14.02-0.97-27.07 5.46-33.5 17.35-19.47 32.56 8.76 96.5 21.81 122.43-15 46.59-33.5 91.04-57.25 134.32-102.82 44.45-104.97 71.54-104.97 81.29 0 11.89 6.43 23.78 18.31 29.24 4.28 3.12 10.71 4.29 16.16 4.29 27.07 0 58.42-30.41 91.92-90.06 42.26-17.35 84.32-31.39 128.73-41.13 22.79 19.49 50.83 30.41 80.04 32.56 18.5 0 54.14 0 53.75-37.04 1.17-14.04-6.43-37.82-57.26-38.99zM353.53 696.45l-3.26 1.17c9.6-14.06 22.26-25 38.38-31.64-8.44 14.26-20.15 25-35.12 30.47z m132.59-322.36c1.02-1.16 3.17-2.35 4.4-2.35l3.57 1.19c5.52 18.41 5.52 37.8-1.23 56.02-7.96-17.26-11.23-36.64-6.74-54.86zM542.26 584c-23.61 5.5-49.55 13.17-73.16 21.81l-2.45 0.96 0.32-1.94c11.81-23.97 22.64-49.13 32.32-74.28l1.16-2.45 1.17 1.47c12 18.47 27.2 37.51 43.46 53.82l-2.82 0.61z m111.17 13.58c-10.87-0.95-21.54-3.04-32.4-8.35 9.7-2.09 18.24-2.09 27.94-2.09 21.54 0 25.81 5.32 26 8.35-6.41 2.09-13.98 3.23-21.54 2.09z"
                fill="#F25555"
            ></path>
            <path
                d="M648.96 575.14c-9.7 0-18.24 0-27.94 2.09 10.87 5.32 21.54 7.4 32.4 8.35 7.57 1.14 15.14 0 21.54-2.09-0.19-3.03-4.46-8.35-26-8.35z m-147.34-57.57l-1.17-1.47-1.16 2.45c-9.68 25.15-20.52 50.31-32.32 74.28l-0.32 1.94 2.45-0.96c23.61-8.65 49.55-16.31 73.16-21.81l2.81-0.61c-16.25-16.32-31.45-35.35-43.45-53.82z m-7.52-156.65l-3.57-1.19c-1.23 0-3.38 1.19-4.4 2.35-4.5 18.22-1.23 37.6 6.75 54.85 6.74-18.21 6.74-37.59 1.22-56.01z m-143.83 324.7l3.26-1.17c14.97-5.47 26.68-16.21 35.12-30.47-16.12 6.64-28.78 17.58-38.38 31.64z m302.48-67c-29.21-2.14-57.25-13.06-80.04-32.56-44.4 9.75-86.47 23.78-128.73 41.13-33.5 59.65-64.85 90.06-91.92 90.06-5.45 0-11.88-1.17-16.16-4.29-11.88-5.46-18.31-17.35-18.31-29.24 0-9.75 2.14-36.84 104.97-81.29 23.76-43.28 42.26-87.73 57.25-134.32-13.05-25.93-41.29-89.87-21.81-122.43 6.43-11.89 19.47-18.32 33.5-17.35 10.91 0 21.62 5.46 28.04 14.04 14.02 19.49 13.05 60.63-5.45 121.26 17.33 32.56 40.12 61.8 66.99 86.56 22.79-4.29 45.38-7.6 68.16-7.6 50.83 1.17 58.42 24.95 57.25 38.99 0.4 37.04-35.24 37.04-53.74 37.04z"
                fill="#FFFFFF"
            ></path>
        </svg>
    }
}
