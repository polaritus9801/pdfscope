//! The AI panel: settings, the question box, and the streamed answer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use pdfscope_core::{Segment, estimate_tokens, link_source_markers, segments, transcript_markdown};

use crate::scope_ui::PageManager;
use crate::settings::SettingsSheet;
use crate::{Passage, Region, truncate};

/// The first question of a conversation, shortened for a list row.
fn conversation_title(turns: &[Turn]) -> String {
    turns
        .first()
        .map(|turn| {
            let question = turn
                .question
                .split("\n\n[引用的先前回答]")
                .next()
                .unwrap_or_default();
            truncate(question, 30)
        })
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| "新对话".to_owned())
}

/// Replaces pill markers with numbered references; an orphaned marker becomes plain text.
fn inline_question(raw: &str, order: &[String]) -> String {
    let mut text = raw.to_owned();
    for (index, key) in order.iter().enumerate() {
        text = text.replace(&format!("@{{{key}}}"), &format!("[引用 {}]", index + 1));
    }
    while let Some(start) = text.find("@{") {
        let Some(offset) = text[start..].find('}') else {
            break;
        };
        text.replace_range(start..start + offset + 1, "[引用已移除]");
    }
    text
}

/// Calls one function on `window.Composer`.
fn composer(name: &str, args: &[JsValue]) -> Option<JsValue> {
    let window = web_sys::window()?;
    let object = js_sys::Reflect::get(&window, &JsValue::from_str("Composer")).ok()?;
    let function = js_sys::Reflect::get(&object, &JsValue::from_str(name)).ok()?;
    let function: js_sys::Function = function.dyn_into().ok()?;
    let arguments = js_sys::Array::new();
    for argument in args {
        arguments.push(argument);
    }
    function.apply(&object, &arguments).ok()
}

/// Calls one function on `window.Bridge`.
pub(crate) fn bridge(name: &str, args: &[JsValue]) -> Option<JsValue> {
    let window = web_sys::window()?;
    let scope = js_sys::Reflect::get(&window, &JsValue::from_str("Bridge")).ok()?;
    let function = js_sys::Reflect::get(&scope, &JsValue::from_str(name)).ok()?;
    let function: js_sys::Function = function.dyn_into().ok()?;

    let arguments = js_sys::Array::new();
    for argument in args {
        arguments.push(argument);
    }

    function.apply(&scope, &arguments).ok()
}

fn field(value: &JsValue, key: &str) -> Option<JsValue> {
    js_sys::Reflect::get(value, &JsValue::from_str(key)).ok()
}

pub(crate) fn text_of(value: &JsValue, key: &str) -> Option<String> {
    field(value, key)?.as_string()
}

pub(crate) fn number_of(value: &JsValue, key: &str) -> Option<f64> {
    field(value, key)?.as_f64()
}

pub(crate) fn flag_of(value: &JsValue, key: &str) -> Option<bool> {
    field(value, key)?.as_bool()
}

/// The preset catalogue, mirroring `pdfscope_ai::PRESETS`.
pub(crate) fn preset(label: &str) -> Option<(&'static str, &'static str)> {
    Some(match label {
        "OpenAI" => ("openai", "https://api.openai.com/v1"),
        "Anthropic" => ("anthropic", "https://api.anthropic.com/v1"),
        "DeepSeek" => ("openai", "https://api.deepseek.com/v1"),
        "MiMo" => ("openai", "https://api.xiaomimimo.com/v1"),
        "Moonshot" => ("openai", "https://api.moonshot.cn/v1"),
        "OpenRouter" => ("openai", "https://openrouter.ai/api/v1"),
        "Gemini" => (
            "openai",
            "https://generativelanguage.googleapis.com/v1beta/openai",
        ),
        "Ollama" => ("openai", "http://127.0.0.1:11434/v1"),
        _ => return None,
    })
}

pub(crate) const PRESET_LABELS: &[&str] = &[
    "MiMo",
    "OpenAI",
    "Anthropic",
    "DeepSeek",
    "Moonshot",
    "OpenRouter",
    "Gemini",
    "Ollama",
];

/// Applies the answer text size to the document root.
pub(crate) fn apply_font_size(size: &str) {
    if let (Ok(size), Some(root)) = (
        size.parse::<u16>(),
        web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element()),
    ) {
        let _ = root.dyn_into::<web_sys::HtmlElement>().map(|element| {
            element
                .style()
                .set_property("--answer-size", &format!("{size}px"))
        });
    }
}

/// Whether the WebView thinks it has no network.
fn offline() -> bool {
    web_sys::window()
        .and_then(|window| {
            js_sys::Reflect::get(&window.into(), &JsValue::from_str("navigator"))
                .ok()
                .and_then(|navigator| {
                    js_sys::Reflect::get(&navigator, &JsValue::from_str("onLine"))
                        .ok()
                        .and_then(|value| value.as_bool())
                })
        })
        .map(|online| !online)
        .unwrap_or(false)
}

/// Copies text through the WebView's clipboard API.
fn copy_to_clipboard(text: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let window: JsValue = window.into();
    let Ok(navigator) = js_sys::Reflect::get(&window, &JsValue::from_str("navigator")) else {
        return;
    };
    let Ok(clipboard) = js_sys::Reflect::get(&navigator, &JsValue::from_str("clipboard")) else {
        return;
    };
    let Ok(write) = js_sys::Reflect::get(&clipboard, &JsValue::from_str("writeText")) else {
        return;
    };
    let Ok(write) = write.dyn_into::<js_sys::Function>() else {
        return;
    };
    let _ = write.call1(&clipboard, &JsValue::from_str(text));
}

/// Applies the chosen theme to the document root.
pub(crate) fn apply_theme(theme: &str) {
    if let Some(root) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.document_element())
    {
        let _ = root.set_attribute("data-theme", theme);
    }
}

/// Fetches a page's text from the renderer.
async fn fetch_page_text(number: u32) -> String {
    let Some(promise) = crate::call("pageText", &[JsValue::from(number)]) else {
        return String::new();
    };
    match JsFuture::from(js_sys::Promise::from(promise)).await {
        Ok(value) => value.as_string().unwrap_or_default(),
        Err(_) => String::new(),
    }
}

/// One web result kept for the citation row.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Source {
    title: String,
    url: String,
}

/// One finished exchange.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub(crate) struct Turn {
    question: String,
    answer: String,
    reasoning: String,
    seconds: f64,
    /// The stream was cut off, so the answer can be continued.
    interrupted: bool,
    /// Web results this answer was allowed to cite.
    #[serde(default)]
    sources: Vec<Source>,
}

/// One conversation stored against a document.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct Conversation {
    id: String,
    title: String,
    updated: f64,
    #[serde(default)]
    pinned: bool,
    /// Model override for this conversation, when the reader picked one.
    #[serde(default)]
    model: Option<String>,
    turns: Vec<Turn>,
}

/// A text file the reader attached to a question.
#[derive(Clone, PartialEq)]
struct UserFile {
    id: u64,
    name: String,
    text: String,
}

/// An image the reader attached to a question.
#[derive(Clone, PartialEq)]
struct UserImage {
    id: u64,
    name: String,
    image: String,
}

/// A reference that was used and moved to history.
#[derive(Clone, PartialEq)]
enum Stored {
    Passage(Passage),
    Region(Region),
}

#[component]
pub fn AiPanel(
    passages: RwSignal<Vec<Passage>>,
    regions: RwSignal<Vec<Region>>,
    page: RwSignal<u32>,
    pages: RwSignal<u32>,
    hovered: RwSignal<Option<(u32, (f64, f64, f64, f64))>>,
    clear_selection: Callback<()>,
    open: RwSignal<bool>,
    notify: Callback<String>,
    configured: RwSignal<bool>,
    undo_push: Callback<()>,
    reset_history: Callback<()>,
    scope_mode: RwSignal<u8>,
    page_set: RwSignal<Vec<u32>>,
    page_limit: RwSignal<u32>,
    context_limit: RwSignal<u32>,
    model_choices: RwSignal<Vec<String>>,
    answers: RwSignal<Vec<crate::AnswerRef>>,
    next_answer: RwSignal<u64>,
    document_key: RwSignal<String>,
) -> impl IntoView {
    let model = RwSignal::new("未配置".to_owned());
    let answer = RwSignal::new(String::new());
    let turns = RwSignal::new(Vec::<Turn>::new());
    let current_question = RwSignal::new(String::new());
    let interrupted = RwSignal::new(false);
    let continue_target = RwSignal::new(None::<usize>);
    let reasoning = RwSignal::new(String::new());
    let think_started = RwSignal::new(0.0_f64);
    let think_seconds = RwSignal::new(0.0_f64);
    let status = RwSignal::new(String::new());
    let streaming = RwSignal::new(false);
    let question = RwSignal::new(String::new());
    let mentions = RwSignal::new(Vec::<String>::new());
    let known_mentions = RwSignal::new(Vec::<String>::new());
    let mention_open = RwSignal::new(false);
    let editing = RwSignal::new(Option::<usize>::None);
    let conversations = RwSignal::new(Vec::<Conversation>::new());
    let active = RwSignal::new(String::new());
    let history_open = RwSignal::new(false);
    let history_query = RwSignal::new(String::new());
    let renaming = RwSignal::new(Option::<(String, String)>::None);
    let confirm_delete = RwSignal::new(Option::<String>::None);
    let sync_ready = RwSignal::new(false);
    let temporary = RwSignal::new(Option::<String>::None);
    let next_conversation = RwSignal::new(0_u64);
    let images = RwSignal::new(Vec::<UserImage>::new());
    let next_image = RwSignal::new(0_u64);
    let files = RwSignal::new(Vec::<UserFile>::new());
    let web = RwSignal::new(false);
    let sources = RwSignal::new(Vec::<Source>::new());
    let pending_clear = RwSignal::new(false);
    let prompt_history = RwSignal::new(Vec::<String>::new());
    let history_cursor = RwSignal::new(Option::<usize>::None);
    let last_asked = RwSignal::new(String::new());
    let session_model = RwSignal::new(Option::<String>::None);
    let request_started = RwSignal::new(0_u64);
    let next_file = RwSignal::new(0_u64);
    let editor_ref = NodeRef::<leptos::html::Div>::new();
    let failed = RwSignal::new(false);
    let history = RwSignal::new(Vec::<Stored>::new());
    let show_all = RwSignal::new(false);
    let notice = RwSignal::new(Option::<String>::None);
    let collapsed = RwSignal::new(false);
    let dragging = RwSignal::new(false);
    let width = RwSignal::new(380_u32);
    let stage_anchor = RwSignal::new(Option::<(f64, f64)>::None);

    // Restore the last width; the divider remembers it per machine.
    Effect::new(move |_| {
        if let Some(width_now) = crate::call("loadPanelWidth", &[]).and_then(|value| value.as_f64())
        {
            if width_now.is_finite() {
                width.set((width_now as u32).clamp(320, 720));
            }
        }
    });
    let pending = RwSignal::new(String::new());
    let scheduled = RwSignal::new(false);

    let jump = move |target: u32| {
        let total = pages.get();
        if total == 0 {
            return;
        }
        clear_selection.run(());
        let target = target.clamp(1, total);
        page.set(target);
        let _ = crate::call("flash", &[JsValue::from(target)]);
    };

    let manager_open = RwSignal::new(false);
    let token_estimate = move || {
        let mut characters = question.get().chars().count();
        for passage in passages.get() {
            characters += passage.quote.chars().count();
        }
        for answer in answers.get() {
            characters += answer.quote.chars().count();
        }
        for file in files.get() {
            characters += file.text.chars().count().min(20_000);
        }
        estimate_tokens(
            characters,
            regions.get().len() + images.get().len(),
            page_set.get().len(),
        )
    };
    let reference_count = move || {
        passages.get().len()
            + regions.get().len()
            + page_set.get().len()
            + images.get().len()
            + files.get().len()
    };

    let redeem = move |index: usize| {
        if let Some(entry) = history.get().get(index).cloned() {
            match entry {
                Stored::Passage(passage) => passages.update(|list| list.push(passage)),
                Stored::Region(region) => regions.update(|list| list.push(region)),
            }
        }
    };

    let settle_seconds = move || {
        if think_seconds.get() == 0.0 && think_started.get() > 0.0 {
            think_seconds.set(((js_sys::Date::now() - think_started.get()) / 1000.0).max(0.1));
        }
    };

    // A finished turn is archived here rather than on the next send: an answer that ends empty
    // (reasoning consumed the budget) must still leave a trace instead of vanishing.
    let archive = move || {
        if current_question.get().trim().is_empty() {
            return;
        }
        if answer.get().trim().is_empty() && reasoning.get().trim().is_empty() {
            return;
        }

        if let Some(index) = continue_target.get() {
            let addition = answer.get();
            let cut = interrupted.get();
            turns.update(|list| {
                if let Some(turn) = list.get_mut(index) {
                    turn.answer.push_str(&addition);
                    turn.interrupted = cut;
                }
            });
            continue_target.set(None);
            current_question.set(String::new());
            answer.set(String::new());
            reasoning.set(String::new());
            return;
        }

        let finished = Turn {
            question: current_question.get(),
            answer: answer.get(),
            reasoning: reasoning.get(),
            seconds: think_seconds.get().max(0.1),
            interrupted: interrupted.get(),
            sources: sources.get(),
        };
        turns.update(|list| list.push(finished));
        // The answer keeps its own copy now; the shelf row is only for the live turn.
        sources.set(Vec::new());
        current_question.set(String::new());
        answer.set(String::new());
        reasoning.set(String::new());
    };

    let draft_key_for = move |id: &str| format!("{}.{}", document_key.get(), id);

    let label_for = move |key: &str| -> Option<String> {
        if let Some(id) = key.strip_prefix("p:").and_then(|id| id.parse::<u64>().ok()) {
            passages
                .get()
                .iter()
                .find(|passage| passage.id == id)
                .map(|passage| format!("p.{}", passage.page))
        } else if let Some(id) = key.strip_prefix("r:").and_then(|id| id.parse::<u64>().ok()) {
            regions
                .get()
                .iter()
                .find(|region| region.id == id)
                .map(|region| format!("图 p.{}", region.page))
        } else if let Some(page) = key
            .strip_prefix("page:")
            .and_then(|page| page.parse::<u32>().ok())
        {
            Some(format!("p.{page}"))
        } else if let Some(id) = key.strip_prefix("a:").and_then(|id| id.parse::<u64>().ok()) {
            answers
                .get()
                .iter()
                .find(|answer| answer.id == id)
                .map(|answer| answer.label.clone())
        } else if let Some(id) = key
            .strip_prefix("img:")
            .and_then(|id| id.parse::<u64>().ok())
        {
            images
                .get()
                .iter()
                .find(|image| image.id == id)
                .map(|image| truncate(&image.name, 14))
        } else if let Some(id) = key
            .strip_prefix("file:")
            .and_then(|id| id.parse::<u64>().ok())
        {
            files
                .get()
                .iter()
                .find(|file| file.id == id)
                .map(|file| truncate(&file.name, 14))
        } else {
            None
        }
    };

    let draft_parts = move |text: &str| -> js_sys::Array {
        let parts = js_sys::Array::new();
        let push_text = |value: &str| {
            if value.is_empty() {
                return;
            }
            let part = js_sys::Object::new();
            let _ =
                js_sys::Reflect::set(&part, &JsValue::from_str("text"), &JsValue::from_str(value));
            parts.push(&part);
        };

        let mut rest = text;
        while let Some(start) = rest.find("@{") {
            push_text(&rest[..start]);
            let after = &rest[start + 2..];
            let Some(end) = after.find('}') else {
                break;
            };
            let key = &after[..end];
            if let Some(label) = label_for(key) {
                let part = js_sys::Object::new();
                let _ =
                    js_sys::Reflect::set(&part, &JsValue::from_str("key"), &JsValue::from_str(key));
                let _ = js_sys::Reflect::set(
                    &part,
                    &JsValue::from_str("label"),
                    &JsValue::from_str(&label),
                );
                parts.push(&part);
            }
            rest = &after[end + 1..];
        }
        push_text(rest);
        parts
    };

    let save_draft_now = move |text: &str| {
        if temporary.get().is_some() {
            return;
        }
        let id = active.get();
        if id.is_empty() {
            return;
        }
        let key = draft_key_for(&id);
        let _ = composer(
            "saveDraftSoon",
            &[JsValue::from_str(&key), JsValue::from_str(text)],
        );
    };

    let restore_draft = move |id: &str| {
        if temporary.get().is_some() || id.is_empty() {
            let _ = composer("clear", &[JsValue::from_bool(true)]);
            question.set(String::new());
            mentions.set(Vec::new());
            known_mentions.set(Vec::new());
            return;
        }
        let key = draft_key_for(id);
        let text = composer("loadDraft", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        let parts = draft_parts(&text);
        let _ = composer("setRich", &[parts.into()]);
    };

    let drop_draft = move |id: &str| {
        if id.is_empty() {
            return;
        }
        let key = draft_key_for(id);
        let _ = composer("removeDraft", &[JsValue::from_str(&key)]);
    };

    // The editor reports text and the attachment keys of its pills.
    Effect::new(move |_| {
        let callback =
            Closure::<dyn Fn(String, JsValue)>::new(move |text: String, keys: JsValue| {
                let current: Vec<String> = keys
                    .dyn_into::<js_sys::Array>()
                    .map(|list| list.iter().filter_map(|value| value.as_string()).collect())
                    .unwrap_or_default();

                let previous = known_mentions.get();
                for key in previous.iter().filter(|key| !current.contains(key)) {
                    if let Some(id) = key.strip_prefix("p:").and_then(|id| id.parse::<u64>().ok()) {
                        undo_push.run(());
                        passages.update(|list| list.retain(|entry| entry.id != id));
                    } else if let Some(id) =
                        key.strip_prefix("r:").and_then(|id| id.parse::<u64>().ok())
                    {
                        undo_push.run(());
                        regions.update(|list| list.retain(|entry| entry.id != id));
                    } else if let Some(page) = key
                        .strip_prefix("page:")
                        .and_then(|page| page.parse::<u32>().ok())
                    {
                        undo_push.run(());
                        page_set.update(|set| set.retain(|entry| *entry != page));
                    } else if let Some(id) =
                        key.strip_prefix("a:").and_then(|id| id.parse::<u64>().ok())
                    {
                        undo_push.run(());
                        answers.update(|list| list.retain(|entry| entry.id != id));
                    } else if let Some(id) = key
                        .strip_prefix("img:")
                        .and_then(|id| id.parse::<u64>().ok())
                    {
                        images.update(|list| list.retain(|entry| entry.id != id));
                    }
                }

                known_mentions.set(current.clone());
                mentions.set(current);
                question.set(text.clone());
                save_draft_now(&text);
            });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeComposerInput"),
                callback.as_ref(),
            );
        }
        callback.forget();
    });

    Effect::new(move |_| {
        let open = Closure::<dyn Fn()>::new(move || mention_open.set(true));
        let close = Closure::<dyn Fn()>::new(move || mention_open.set(false));
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeMentionOpen"),
                open.as_ref(),
            );
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopeMentionClose"),
                close.as_ref(),
            );
        }
        open.forget();
        close.forget();
    });

    Effect::new(move |_| {
        let _ = mention_open.get();
        let _ = composer("mentionReset", &[]);
    });

    Effect::new(move |_| {
        if let Some(json) = composer("loadPromptHistory", &[]).and_then(|value| value.as_string()) {
            if let Ok(saved) = serde_json::from_str::<Vec<String>>(&json) {
                prompt_history.set(saved.into_iter().take(30).collect());
            }
        }
    });

    // Hand the editor element to the renderer once it exists.
    Effect::new(move |_| {
        let Some(element) = editor_ref.get() else {
            return;
        };
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(composer) = js_sys::Reflect::get(&window, &JsValue::from_str("Composer")) else {
            return;
        };
        let Ok(init) = js_sys::Reflect::get(&composer, &JsValue::from_str("init")) else {
            return;
        };
        if let Ok(init) = init.dyn_into::<js_sys::Function>() {
            let _ = init.call1(&composer, &element);
        }
    });

    let new_conversation_id = move || {
        let serial = next_conversation.get() + 1;
        next_conversation.set(serial);
        format!("{}-{serial}", js_sys::Date::now() as u64)
    };

    let save_conversations = move || {
        let key = document_key.get_untracked();
        if key.trim().is_empty() {
            return;
        }
        if let Ok(json) = serde_json::to_string(&conversations.get_untracked()) {
            let _ = crate::call(
                "saveConversations",
                &[JsValue::from_str(&key), JsValue::from_str(&json)],
            );
        }
    };

    // A document owns a library of conversations; the first one is selected on open.
    Effect::new(move |_| {
        let key = document_key.get();
        sync_ready.set(false);
        if key.trim().is_empty() {
            conversations.set(Vec::new());
            active.set(String::new());
            turns.set(Vec::new());
            images.set(Vec::new());
            temporary.set(None);
            sync_ready.set(true);
            return;
        }

        let mut list: Vec<Conversation> =
            crate::call("loadConversations", &[JsValue::from_str(&key)])
                .and_then(|value| value.as_string())
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();

        if list.is_empty() {
            // Migrate the single transcript written by the previous build.
            let legacy: Vec<Turn> = crate::call("loadTranscript", &[JsValue::from_str(&key)])
                .and_then(|value| value.as_string())
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            let id = new_conversation_id();
            list.push(Conversation {
                title: conversation_title(&legacy),
                id: id.clone(),
                updated: js_sys::Date::now(),
                pinned: false,
                model: None,
                turns: legacy,
            });
        }

        list.sort_by(|left, right| {
            right.pinned.cmp(&left.pinned).then_with(|| {
                right
                    .updated
                    .partial_cmp(&left.updated)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        let first = list[0].id.clone();
        let first_turns = list[0].turns.clone();
        let first_model = list[0].model.clone();
        conversations.set(list);
        active.set(first.clone());
        turns.set(first_turns);
        session_model.set(first_model);
        restore_draft(&first);
        show_all.set(false);
        sync_ready.set(true);
        save_conversations();
    });

    // Whatever changes the active turns is written back into its conversation.
    Effect::new(move |_| {
        if !sync_ready.get() {
            return;
        }
        let current_turns = turns.get();
        let id = active.get();
        if id.is_empty() || temporary.get().is_some() {
            return;
        }
        conversations.update(|list| {
            if let Some(conversation) = list.iter_mut().find(|entry| entry.id == id) {
                if conversation.turns != current_turns {
                    conversation.updated = js_sys::Date::now();
                    if conversation.title.trim().is_empty() || conversation.title == "新对话" {
                        conversation.title = conversation_title(&current_turns);
                    }
                    conversation.turns = current_turns.clone();
                }
            }
        });
        save_conversations();
    });

    let send_text = move |asked: String| {
        if streaming.get() {
            return;
        }
        if asked.trim().is_empty() {
            return;
        }
        if !configured.get() {
            open.set(true);
            return;
        }

        if continue_target.get().is_none() {
            interrupted.set(false);
        }
        current_question.set(asked.clone());
        answer.set(String::new());
        reasoning.set(String::new());
        think_started.set(js_sys::Date::now());
        think_seconds.set(0.0);
        streaming.set(true);
        failed.set(false);
        status.set("准备上下文…".to_owned());
        let web_now = web.get();
        sources.set(Vec::new());

        let passages_now = passages.get();
        let regions_now = regions.get();

        let current_page = page.get();
        let total = pages.get();
        let pictures = images.get();
        let model_override = session_model.get();
        let mode = scope_mode.get();
        let explicit = page_set.get();

        spawn_local(async move {
            let mut model_question = asked.clone();
            if web_now {
                status.set("联网搜索中…".to_owned());
                let Some(promise) = bridge("search", &[JsValue::from_str(&asked)]) else {
                    status.set("联网搜索不可用：这是浏览器版本。".to_owned());
                    failed.set(true);
                    streaming.set(false);
                    return;
                };
                match JsFuture::from(js_sys::Promise::from(promise)).await {
                    Ok(value) => {
                        let mut found = Vec::new();
                        if let Ok(list) = value.dyn_into::<js_sys::Array>() {
                            for entry in list.iter() {
                                let title =
                                    crate::model::read_string(&entry, "title").unwrap_or_default();
                                let url =
                                    crate::model::read_string(&entry, "url").unwrap_or_default();
                                if url.is_empty() {
                                    continue;
                                }
                                let content = crate::model::read_string(&entry, "content")
                                    .unwrap_or_default();
                                found.push(Source {
                                    title: title.clone(),
                                    url: url.clone(),
                                });
                                model_question.push_str(&format!(
                                    "\n\n[来源 {}] {}\n{}\n{}",
                                    found.len(),
                                    title,
                                    url,
                                    content
                                ));
                            }
                        }
                        if found.is_empty() {
                            status.set("没有搜到结果，继续用本地上下文。".to_owned());
                        } else {
                            sources.set(found);
                            status.set("已联网，正在整理…".to_owned());
                        }
                    }
                    Err(error) => {
                        let message = error
                            .as_string()
                            .unwrap_or_else(|| "请检查搜索 API Key".to_owned());
                        status.set(format!("联网搜索失败：{message}"));
                        failed.set(true);
                        streaming.set(false);
                        pending_clear.set(false);
                        return;
                    }
                }
            }

            // Page text is fetched here, not in Rust: the renderer owns the document.
            let mut context: Vec<(u32, String)> = Vec::new();

            let cap = match page_limit.get() {
                0 => None,
                limit => Some(limit as usize),
            };
            // "References" means everything the reader marked: page bars as well as quotes and boxes.
            let scope = match mode {
                0 => pdfscope_core::pages::Scope::Document,
                1 => pdfscope_core::pages::Scope::Current,
                _ => pdfscope_core::pages::Scope::Selected,
            };
            let (selected, wanted) =
                pdfscope_core::pages::request_pages(scope, total, current_page, &explicit, cap);

            if mode == 2 && explicit.is_empty() && passages_now.is_empty() && regions_now.is_empty()
            {
                notice.set(Some("还没有引用：这次只问问题，不发送页面".to_owned()));
            } else if wanted > selected.len() {
                let label = if mode == 0 {
                    "整份文档"
                } else {
                    "已选页面"
                };
                notice.set(Some(format!(
                    "{label}共 {wanted} 页，本次只发送前 {} 页（可在设置里调大）",
                    selected.len()
                )));
            } else {
                notice.set(None);
            }

            for number in &selected {
                context.push((*number, fetch_page_text(*number).await));
            }

            // Check the real size of what is about to be sent, including page text fetched above.
            let mut estimated_chars = asked.chars().count();
            for (_, text) in &context {
                estimated_chars += text.chars().count();
            }
            for passage in &passages_now {
                estimated_chars += passage.quote.chars().count();
            }
            let estimate = estimate_tokens(estimated_chars, regions_now.len() + pictures.len(), 0);
            let limit = context_limit.get();
            if limit > 0 && estimate > limit as usize {
                status.set(format!(
                    "上下文约 {estimate} tokens，超过上限 {limit}；请减少页面或引用，或在设置里调大"
                ));
                failed.set(true);
                streaming.set(false);
                pending_clear.set(false);
                return;
            }

            // Only a request that actually starts consumes the references. A failed search or a
            // guard trip must leave them in the shelf so the reader can retry without re-selecting.
            if !passages_now.is_empty() || !regions_now.is_empty() {
                history.update(|list| {
                    list.extend(passages_now.iter().cloned().map(Stored::Passage));
                    list.extend(regions_now.iter().cloned().map(Stored::Region));
                });
                passages.set(Vec::new());
                regions.set(Vec::new());
                reset_history.run(());
            }
            request_started.update(|count| *count += 1);

            let references = js_sys::Array::new();
            for passage in &passages_now {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("page"),
                    &JsValue::from(passage.page),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("quote"),
                    &JsValue::from_str(&passage.quote),
                );
                references.push(&item);
            }

            let images = js_sys::Array::new();
            for region in &regions_now {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("page"),
                    &JsValue::from(region.page),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("image"),
                    &JsValue::from_str(&region.image),
                );
                images.push(&item);
            }
            for picture in &pictures {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(&item, &JsValue::from_str("page"), &JsValue::from(0));
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("image"),
                    &JsValue::from_str(&picture.image),
                );
                images.push(&item);
            }

            let pages_array = js_sys::Array::new();
            for (number, text) in &context {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("page"),
                    &JsValue::from(*number),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("text"),
                    &JsValue::from_str(text),
                );
                pages_array.push(&item);
            }

            let history = js_sys::Array::new();
            let recent: Vec<Turn> = turns
                .get()
                .iter()
                .rev()
                .take(4)
                .cloned()
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            for turn in recent {
                let item = js_sys::Object::new();
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("question"),
                    &JsValue::from_str(&turn.question),
                );
                let _ = js_sys::Reflect::set(
                    &item,
                    &JsValue::from_str("answer"),
                    &JsValue::from_str(&turn.answer),
                );
                history.push(&item);
            }

            status.set("等待回答…".to_owned());
            let started = js_sys::Date::now();
            let callback = Closure::<dyn Fn(JsValue)>::new(move |event: JsValue| {
                let flush_pending = move || {
                    let text = pending.get();
                    pending.set(String::new());
                    if !text.is_empty() {
                        answer.update(|buffer| buffer.push_str(&text));
                    }
                };
                let kind = text_of(&event, "kind").unwrap_or_default();
                match kind.as_str() {
                    "reasoning" => {
                        if let Some(delta) = text_of(&event, "text") {
                            if think_started.get() == 0.0 {
                                think_started.set(js_sys::Date::now());
                            }
                            reasoning.update(|buffer| buffer.push_str(&delta));
                        }
                    }
                    "text" => {
                        status.update(|line| {
                            if line.contains("正在重试") {
                                line.clear();
                            }
                        });
                        if think_seconds.get() == 0.0 && think_started.get() > 0.0 {
                            think_seconds.set((js_sys::Date::now() - think_started.get()) / 1000.0);
                        }
                        if let Some(delta) = text_of(&event, "text") {
                            // Coalesce tokens into one reactive update per frame: the answer
                            // re-parses citations on every change, so per-token updates are O(n²).
                            pending.update(|buffer| buffer.push_str(&delta));
                            if !scheduled.get() {
                                scheduled.set(true);
                                let flush = Closure::once(move || {
                                    scheduled.set(false);
                                    let text = pending.get();
                                    pending.set(String::new());
                                    if !text.is_empty() {
                                        answer.update(|buffer| buffer.push_str(&text));
                                    }
                                });
                                if let Some(window) = web_sys::window() {
                                    let _ = window
                                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                                            flush.as_ref().unchecked_ref(),
                                            50,
                                        );
                                }
                                flush.forget();
                            }
                        }
                    }
                    "retrying" => {
                        let attempt = number_of(&event, "attempt").unwrap_or(0.0) as u32;
                        let of = number_of(&event, "of").unwrap_or(0.0) as u32;
                        status.set(format!("连接中断，正在重试（{attempt}/{of}）…"));
                    }
                    "usage" => {
                        let prompt = number_of(&event, "prompt").unwrap_or(0.0) as u64;
                        let completion = number_of(&event, "completion").unwrap_or(0.0) as u64;
                        let cached = number_of(&event, "cached").unwrap_or(0.0) as u64;
                        status.set(format!("输入 {prompt} · 输出 {completion} · 缓存 {cached}"));
                    }
                    "stopped" => {
                        flush_pending();
                        streaming.set(false);
                        settle_seconds();
                        let kept = !answer.get().trim().is_empty();
                        archive();
                        status.set(if kept {
                            "已停止".to_owned()
                        } else {
                            "已停止，没有生成正文".to_owned()
                        });
                    }
                    "done" => {
                        flush_pending();
                        streaming.set(false);
                        settle_seconds();

                        let elapsed = js_sys::Date::now() - started;
                        let characters = answer.get().chars().count() as f64;
                        let empty = answer.get().trim().is_empty();
                        let had_reasoning = !reasoning.get().trim().is_empty();
                        archive();

                        if empty {
                            status.set(if had_reasoning {
                                "模型只返回了思考过程，没有正文".to_owned()
                            } else {
                                "模型没有返回内容".to_owned()
                            });
                        } else if elapsed > 0.0 {
                            let rate = (characters / 2.0) / (elapsed / 1000.0);
                            status.update(|line| line.push_str(&format!(" · ≈ {rate:.0} tok/s")));
                        }
                    }
                    "error" => {
                        flush_pending();
                        streaming.set(false);
                        settle_seconds();
                        interrupted.set(!answer.get().trim().is_empty());
                        archive();
                        failed.set(true);
                        let message =
                            text_of(&event, "message").unwrap_or_else(|| "请求失败".to_owned());
                        status.set(if offline() {
                            "网络已断开，请检查连接后再试。".to_owned()
                        } else {
                            message
                        });
                    }
                    _ => {}
                }
            });

            let model_value = model_override
                .clone()
                .map(|name| JsValue::from_str(&name))
                .unwrap_or(JsValue::NULL);
            let _ = bridge(
                "ask",
                &[
                    JsValue::from_str(&model_question),
                    references.into(),
                    images.into(),
                    pages_array.into(),
                    history.into(),
                    model_value,
                    callback.as_ref().clone(),
                ],
            );
            callback.forget();
        });
    };

    Effect::new(move |_| {
        if !manager_open.get() {
            return;
        }
        if let Some(window) = web_sys::window() {
            let here = page.get();
            let schedule = Closure::once_into_js(move || {
                let _ = crate::call("thumbnails", &[]);
                let _ = crate::call(
                    "centerThumb",
                    &[
                        JsValue::from_str("manager-grid"),
                        JsValue::from_f64(f64::from(here)),
                    ],
                );
            });
            let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
                schedule.unchecked_ref(),
                80,
            );
        }
    });

    Effect::new(move |_| {
        let handler =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                if event.key() == "Escape" && manager_open.get() {
                    manager_open.set(false);
                }
            });
        if let Some(window) = web_sys::window() {
            let _ = window
                .add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        }
        handler.forget();
    });

    let composer_clear = move || {
        let _ = composer("clear", &[JsValue::from_bool(true)]);
        question.set(String::new());
        mentions.set(Vec::new());
        known_mentions.set(Vec::new());
    };

    // The composer is cleared only once a request has actually started, not when the button was
    // pressed: a failed web search must leave the draft and its attachments untouched.
    Effect::new(move |_| {
        let _ = request_started.get();
        if !pending_clear.get() {
            return;
        }
        pending_clear.set(false);
        let asked = last_asked.get();
        if !asked.trim().is_empty() && prompt_history.get().last() != Some(&asked) {
            prompt_history.update(|list| {
                list.push(asked.clone());
                while list.len() > 30 {
                    list.remove(0);
                }
            });
            if let Ok(json) = serde_json::to_string(prompt_history.get_untracked().as_slice()) {
                let _ = composer("savePromptHistory", &[JsValue::from_str(&json)]);
            }
        }
        history_cursor.set(None);
        answers.set(Vec::new());
        images.set(Vec::new());
        files.set(Vec::new());
        let sent_id = active.get();
        drop_draft(&sent_id);
        composer_clear();
    });

    let send = move |_| {
        // While an answer streams the button is "stop"; Enter must not wipe the draft either.
        if streaming.get() {
            return;
        }
        let mut order = Vec::new();
        for passage in passages.get() {
            order.push(format!("p:{}", passage.id));
        }
        for region in regions.get() {
            order.push(format!("r:{}", region.id));
        }
        for page in page_set.get() {
            order.push(format!("page:{page}"));
        }
        for answer in answers.get() {
            order.push(format!("a:{}", answer.id));
        }

        let mut text = inline_question(&question.get(), &order);
        if !answers.get().is_empty() {
            text.push_str("\n\n[引用的先前回答]\n");
            for (index, answer) in answers.get().iter().enumerate() {
                text.push_str(&format!(
                    "回答 {}：{}\n",
                    index + 1,
                    truncate(&answer.quote, 4000)
                ));
            }
        }
        for file in files.get() {
            text.push_str(&pdfscope_core::file_block(&file.name, &file.text, 20_000));
            text.push('\n');
        }
        if text.trim().is_empty() {
            return;
        }
        if let Some(index) = editing.get() {
            turns.update(|list| list.truncate(index));
            editing.set(None);
        }
        last_asked.set(text.clone());
        send_text(text);
        // Only a request that really started should clear the field; an early return (not
        // configured, empty question) must leave the draft alone.
        if streaming.get() {
            pending_clear.set(true);
        }
    };

    let continue_last = move || {
        let all = turns.get();
        let Some(last) = all.len().checked_sub(1) else {
            return;
        };
        continue_target.set(Some(last));
        send_text("请从断开处继续写下去，不要重复已经写过的内容。".to_owned());
    };

    // Global shortcuts for the answer actions; they ignore plain typing so the composer is unaffected.
    Effect::new(move |_| {
        let Some(window) = web_sys::window() else {
            return;
        };

        let handler =
            Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(move |event: web_sys::KeyboardEvent| {
                if !(event.meta_key() || event.ctrl_key()) || !event.shift_key() {
                    return;
                }

                match event.key().to_ascii_lowercase().as_str() {
                    "c" => {
                        let text = turns
                            .get()
                            .last()
                            .map(|turn| turn.answer.clone())
                            .filter(|text| !text.trim().is_empty())
                            .unwrap_or_else(|| answer.get());
                        if text.trim().is_empty() {
                            return;
                        }
                        copy_to_clipboard(&text);
                        notify.run("已复制".to_owned());
                        event.prevent_default();
                    }
                    "r" => {
                        let Some(last) = turns.get().last().cloned() else {
                            return;
                        };
                        send_text(last.question);
                        event.prevent_default();
                    }
                    "g" => {
                        let interrupted = turns.get().last().is_some_and(|turn| turn.interrupted);
                        if interrupted {
                            continue_last();
                            event.prevent_default();
                        }
                    }
                    _ => {}
                }
            });

        let _ =
            window.add_event_listener_with_callback("keydown", handler.as_ref().unchecked_ref());
        handler.forget();
    });

    let remove_passage = move |id: u64| {
        let key = format!("p:{id}");
        let removed = composer("removeToken", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !removed {
            undo_push.run(());
            passages.update(|list| list.retain(|entry| entry.id != id));
        }
    };
    let remove_region = move |id: u64| {
        let key = format!("r:{id}");
        let removed = composer("removeToken", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !removed {
            undo_push.run(());
            regions.update(|list| list.retain(|entry| entry.id != id));
        }
    };
    let remove_page = move |page_number: u32| {
        let key = format!("page:{page_number}");
        let removed = composer("removeToken", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !removed {
            undo_push.run(());
            page_set.update(|set| set.retain(|entry| *entry != page_number));
        }
    };
    let remove_file = move |id: u64| {
        let key = format!("file:{id}");
        let removed = composer("removeToken", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !removed {
            files.update(|list| list.retain(|entry| entry.id != id));
        }
    };
    let remove_image = move |id: u64| {
        let key = format!("img:{id}");
        let removed = composer("removeToken", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !removed {
            images.update(|list| list.retain(|entry| entry.id != id));
        }
    };
    let remove_answer = move |id: u64| {
        let key = format!("a:{id}");
        let removed = composer("removeToken", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        if !removed {
            undo_push.run(());
            answers.update(|list| list.retain(|entry| entry.id != id));
        }
    };

    let reset_transient = move || {
        current_question.set(String::new());
        answer.set(String::new());
        reasoning.set(String::new());
        failed.set(false);
        show_all.set(false);
        editing.set(None);
        question.set(String::new());
        mentions.set(Vec::new());
        known_mentions.set(Vec::new());
        sources.set(Vec::new());
        let _ = composer("clear", &[JsValue::from_bool(true)]);
    };

    let switch_conversation = move |id: String| {
        if streaming.get() {
            notify.run("回答生成中，结束后再切换对话".to_owned());
            return;
        }
        let Some(conversation) = conversations.get().into_iter().find(|entry| entry.id == id)
        else {
            return;
        };
        active.set(id.clone());
        session_model.set(conversation.model.clone());
        turns.set(conversation.turns);
        temporary.set(None);
        reset_transient();
        restore_draft(&id);
        history_open.set(false);
        sync_ready.set(true);
    };

    let create_conversation = move || {
        if streaming.get() {
            return;
        }
        let id = new_conversation_id();
        conversations.update(|list| {
            list.push(Conversation {
                id: id.clone(),
                title: "新对话".to_owned(),
                updated: js_sys::Date::now(),
                pinned: false,
                model: None,
                turns: Vec::new(),
            });
        });
        active.set(id.clone());
        session_model.set(None);
        turns.set(Vec::new());
        temporary.set(None);
        reset_transient();
        restore_draft(&id);
        history_open.set(false);
        save_conversations();
    };

    let delete_conversation = move |id: String| {
        if streaming.get() {
            return;
        }
        drop_draft(&id);
        conversations.update(|list| list.retain(|entry| entry.id != id));
        confirm_delete.set(None);
        if active.get() == id {
            let mut remaining = conversations.get();
            remaining.sort_by(|left, right| {
                right.pinned.cmp(&left.pinned).then_with(|| {
                    right
                        .updated
                        .partial_cmp(&left.updated)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
            if let Some(next) = remaining.into_iter().next() {
                active.set(next.id.clone());
                session_model.set(next.model.clone());
                turns.set(next.turns);
                reset_transient();
                restore_draft(&next.id);
            } else {
                create_conversation();
            }
        }
        save_conversations();
    };

    let start_temporary = move || {
        if streaming.get() {
            return;
        }
        let id = format!("temp-{}", js_sys::Date::now() as u64);
        temporary.set(Some(id.clone()));
        active.set(id);
        session_model.set(None);
        turns.set(Vec::new());
        reset_transient();
        history_open.set(false);
    };

    let branch_conversation = move |index: usize| {
        if streaming.get() {
            notify.run("回答生成中，结束后再分支".to_owned());
            return;
        }
        let prefix: Vec<Turn> = turns.get().into_iter().take(index + 1).collect();
        if prefix.is_empty() {
            return;
        }
        let parent_title = conversations
            .get()
            .into_iter()
            .find(|conversation| conversation.id == active.get())
            .map(|conversation| conversation.title)
            .unwrap_or_else(|| "对话".to_owned());
        let id = new_conversation_id();
        conversations.update(|list| {
            list.push(Conversation {
                title: format!("{parent_title} · 分支"),
                id: id.clone(),
                updated: js_sys::Date::now(),
                pinned: false,
                model: session_model.get(),
                turns: prefix.clone(),
            });
        });
        temporary.set(None);
        active.set(id);
        turns.set(prefix);
        reset_transient();
        save_conversations();
        notify.run("已从这条回答分支".to_owned());
    };

    let toggle_pin = move |id: String| {
        conversations.update(|list| {
            if let Some(conversation) = list.iter_mut().find(|entry| entry.id == id) {
                conversation.pinned = !conversation.pinned;
            }
        });
        save_conversations();
    };

    let commit_rename = move |id: String, title: String| {
        let title = if title.trim().is_empty() {
            "新对话".to_owned()
        } else {
            title.trim().to_owned()
        };
        conversations.update(|list| {
            if let Some(conversation) = list.iter_mut().find(|entry| entry.id == id) {
                conversation.title = title;
                conversation.updated = js_sys::Date::now();
            }
        });
        renaming.set(None);
        save_conversations();
    };

    view! {
        <aside
            class="panel"
            class:collapsed=move || collapsed.get()
            style=move || {
                if collapsed.get() {
                    "width: 44px".to_owned()
                } else {
                    format!("width: {}px", width.get())
                }
            }
        >
            <div
                class="panel-resize"
                on:pointerdown=move |event: web_sys::PointerEvent| {
                    event.prevent_default();
                    if let Some(target) = event
                        .current_target()
                        .and_then(|target| target.dyn_into::<web_sys::HtmlElement>().ok())
                    {
                        let _ = target.set_pointer_capture(event.pointer_id());
                    }
                    // Without this a fast drag starts selecting text on both sides of the split.
                    if let Some(root) = web_sys::window()
                        .and_then(|window| window.document())
                        .and_then(|document| document.document_element())
                    {
                        let _ = root.class_list().add_1("resizing");
                    }
                    let _ = crate::call("clearSelection", &[]);
                    if let Some(anchor) = crate::call("captureStageAnchor", &[]) {
                        let x = js_sys::Reflect::get(&anchor, &JsValue::from_str("x"))
                            .ok()
                            .and_then(|value| value.as_f64());
                        let y = js_sys::Reflect::get(&anchor, &JsValue::from_str("y"))
                            .ok()
                            .and_then(|value| value.as_f64());
                        stage_anchor.set(x.zip(y));
                    }
                    dragging.set(true);
                }
                on:pointermove=move |event: web_sys::PointerEvent| {
                    if !dragging.get() {
                        return;
                    }
                    event.prevent_default();
                    let viewport = web_sys::window()
                        .and_then(|window| window.inner_width().ok())
                        .and_then(|value| value.as_f64())
                        .unwrap_or(1280.0);
                    let next = (viewport - f64::from(event.client_x())).clamp(320.0, 720.0);
                    width.set(next as u32);
                    if let Some((x, y)) = stage_anchor.get() {
                        let _ = crate::call(
                            "restoreStageAnchor",
                            &[JsValue::from_f64(x), JsValue::from_f64(y)],
                        );
                    }
                }
                on:dblclick=move |_| {
                    width.set(380);
                    let _ = crate::call("savePanelWidth", &[JsValue::from_f64(380.0)]);
                }
                on:pointerup=move |_| {
                    dragging.set(false);
                    stage_anchor.set(None);
                    let _ = crate::call(
                        "savePanelWidth",
                        &[JsValue::from_f64(f64::from(width.get()))],
                    );
                    if let Some(root) = web_sys::window()
                        .and_then(|window| window.document())
                        .and_then(|document| document.document_element())
                    {
                        let _ = root.class_list().remove_1("resizing");
                    }
                }
            ></div>
            <div class="panel-head">
                <button
                    class="panel-toggle"
                    title="收起"
                    on:click=move |_| collapsed.set(true)
                >
                    <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M9 6l6 6-6 6" />
                    </svg>
                </button>
                <span class="panel-title">"AI"</span>
                {move || {
                    let choices = model_choices.get();
                    if choices.is_empty() {
                        return view! { <span class="panel-model">{model.get()}</span> }.into_any();
                    }
                    let current = session_model.get().unwrap_or_default();
                    let options = choices
                        .into_iter()
                        .map(|choice| {
                            let selected = choice == current;
                            let value = choice.clone();
                            let label = choice.clone();
                            view! {
                                <option value=value selected=selected>
                                    {label}
                                </option>
                            }
                        })
                        .collect_view();
                    view! {
                        <select
                            class="panel-model-select"
                            title="本对话使用的模型"
                            on:change=move |event| {
                                let value = event_target_value(&event);
                                let next = if value.is_empty() { None } else { Some(value) };
                                session_model.set(next.clone());
                                let id = active.get();
                                conversations.update(|list| {
                                    if let Some(conversation) =
                                        list.iter_mut().find(|entry| entry.id == id)
                                    {
                                        conversation.model = next;
                                        conversation.updated = js_sys::Date::now();
                                    }
                                });
                                save_conversations();
                            }
                        >
                            <option value="" selected=current.is_empty()>
                                {format!("默认 · {}", model.get())}
                            </option>
                            {options}
                        </select>
                    }
                        .into_any()
                }}
                <Show when=move || temporary.get().is_some()>
                    <span class="temp-badge" title="这个对话不会被保存">"临时"</span>
                </Show>
                <button
                    class="panel-new"
                    title="历史对话"
                    on:click=move |_| history_open.set(true)
                >
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                        <path d="M3 12a9 9 0 1 0 3-6.7" />
                        <path d="M3 4v5h5" />
                        <path d="M12 8v4l3 2" />
                    </svg>
                </button>
                <button
                    class="panel-new"
                    title="新对话"
                    on:click=move |_| create_conversation()
                >
                    <svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round">
                        <path d="M12 5v14M5 12h14" />
                    </svg>
                </button>
            </div>

            <Show when=move || history_open.get()>
                <div class="history-drawer">
                    <header class="history-head">
                        <span class="history-heading">"对话"</span>
                        <button
                            class="icon-button"
                            title="关闭"
                            on:click=move |_| history_open.set(false)
                        >
                            "✕"
                        </button>
                    </header>
                    <div class="history-tools">
                        <input
                            class="history-search"
                            type="search"
                            placeholder="搜索对话"
                            prop:value=move || history_query.get()
                            on:input=move |event| history_query.set(event_target_value(&event))
                        />
                        <button class="button button-primary" on:click=move |_| create_conversation()>
                            "新对话"
                        </button>
                        <button
                            class="button"
                            title="临时对话：关闭后不保留"
                            on:click=move |_| start_temporary()
                        >
                            "临时"
                        </button>
                        <button
                            class="button"
                            title="把当前对话导出为 Markdown 文件"
                            on:click=move |_| {
                                let pairs: Vec<(String, String)> = conversations
                                    .get()
                                    .into_iter()
                                    .find(|conversation| conversation.id == active.get())
                                    .map(|conversation| {
                                        conversation
                                            .turns
                                            .iter()
                                            .map(|turn| (turn.question.clone(), turn.answer.clone()))
                                            .collect()
                                    })
                                    .unwrap_or_default();
                                let markdown = transcript_markdown(&pairs);
                                if markdown.is_empty() {
                                    notify.run("当前对话还没有内容".to_owned());
                                    return;
                                }
                                let title = conversations
                                    .get()
                                    .into_iter()
                                    .find(|conversation| conversation.id == active.get())
                                    .map(|conversation| conversation.title)
                                    .unwrap_or_else(|| "对话".to_owned());
                                let filename = format!("{title}.md");
                                let Some(promise) = bridge(
                                    "exportMarkdown",
                                    &[JsValue::from_str(&filename), JsValue::from_str(&markdown)],
                                ) else {
                                    copy_to_clipboard(&markdown);
                                    notify.run("已复制 Markdown".to_owned());
                                    return;
                                };
                                spawn_local(async move {
                                    match JsFuture::from(js_sys::Promise::from(promise)).await {
                                        Ok(value) => {
                                            let path = value.as_string().unwrap_or_default();
                                            notify.run(format!("已导出到 {path}"));
                                        }
                                        Err(_) => {
                                            copy_to_clipboard(&markdown);
                                            notify.run("文件导出不可用，已复制 Markdown".to_owned());
                                        }
                                    }
                                });
                            }
                        >
                            "导出"
                        </button>
                    </div>
                    <div class="history-list">
                        {move || {
                            let query = history_query.get().to_lowercase();
                            let mut list = conversations.get();
                            list.sort_by(|left, right| {
                                right
                                    .pinned
                                    .cmp(&left.pinned)
                                    .then_with(|| {
                                        right
                                            .updated
                                            .partial_cmp(&left.updated)
                                            .unwrap_or(std::cmp::Ordering::Equal)
                                    })
                            });
                            if !query.is_empty() {
                                list.retain(|conversation| {
                                    conversation.title.to_lowercase().contains(&query)
                                        || conversation.turns.iter().any(|turn| {
                                            turn.question.to_lowercase().contains(&query)
                                                || turn.answer.to_lowercase().contains(&query)
                                        })
                                });
                            }
                            if list.is_empty() {
                                return view! { <p class="history-empty">"没有找到对话"</p> }
                                    .into_any();
                            }

                            list.into_iter()
                                .map(|conversation| {
                                    let id = conversation.id.clone();
                                    let title = conversation.title.clone();
                                    let pinned = conversation.pinned;
                                    let count = conversation.turns.len();
                                    let is_active = active.get() == id;
                                    let is_renaming = renaming
                                        .get()
                                        .map(|(renaming_id, _)| renaming_id == id)
                                        .unwrap_or(false);
                                    let rename_value = renaming
                                        .get()
                                        .filter(|(renaming_id, _)| *renaming_id == id)
                                        .map(|(_, value)| value)
                                        .unwrap_or_default();
                                    let deleting = confirm_delete.get() == Some(id.clone());

                                    let id_for_open = id.clone();
                                    let id_for_pin = id.clone();
                                    let id_for_rename = id.clone();
                                    let title_for_rename = title.clone();
                                    let id_for_input = id.clone();
                                    let id_for_key = id.clone();
                                    let id_for_blur = id.clone();
                                    let id_for_delete = id.clone();
                                    let id_for_confirm = id.clone();

                                    view! {
                                        <div class="history-row" class:active=is_active>
                                            <button
                                                class="history-pin"
                                                title=if pinned { "取消置顶" } else { "置顶" }
                                                on:click=move |_| toggle_pin(id_for_pin.clone())
                                            >
                                                {if pinned { "★" } else { "☆" }}
                                            </button>
                                            {if is_renaming {
                                                view! {
                                                    <input
                                                        class="history-rename"
                                                        prop:value=rename_value
                                                        on:input=move |event: web_sys::Event| {
                                                            renaming.set(Some((
                                                                id_for_input.clone(),
                                                                event_target_value(&event),
                                                            )));
                                                        }
                                                        on:keydown=move |event: web_sys::KeyboardEvent| {
                                                            if event.key() == "Enter" {
                                                                let value = renaming
                                                                    .get()
                                                                    .map(|(_, value)| value)
                                                                    .unwrap_or_default();
                                                                commit_rename(id_for_key.clone(), value);
                                                            } else if event.key() == "Escape" {
                                                                renaming.set(None);
                                                            }
                                                        }
                                                        on:blur=move |_| {
                                                            // Enter already committed and closed the field; a blur
                                                            // that arrives afterwards must not rename again with "".
                                                            let Some((current, value)) = renaming.get() else {
                                                                return;
                                                            };
                                                            if current != id_for_blur {
                                                                return;
                                                            }
                                                            commit_rename(current, value);
                                                        }
                                                    />
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <button
                                                        class="history-open"
                                                        on:click=move |_| switch_conversation(id_for_open.clone())
                                                    >
                                                        <span class="history-name">{title.clone()}</span>
                                                        <span class="history-count">
                                                            {format!("{count} 轮")}
                                                        </span>
                                                    </button>
                                                }
                                                    .into_any()
                                            }}
                                            <button
                                                class="history-act"
                                                title="重命名"
                                                on:click=move |_| {
                                                    renaming.set(Some((
                                                        id_for_rename.clone(),
                                                        title_for_rename.clone(),
                                                    )));
                                                }
                                            >
                                                "✎"
                                            </button>
                                            <button
                                                class="history-act danger"
                                                class:confirming=deleting
                                                title=if deleting { "再点一次删除" } else { "删除" }
                                                on:click=move |_| {
                                                    if confirm_delete.get() == Some(id_for_delete.clone()) {
                                                        delete_conversation(id_for_delete.clone());
                                                    } else {
                                                        confirm_delete.set(Some(id_for_confirm.clone()));
                                                    }
                                                }
                                            >
                                                {if deleting { "确认" } else { "✕" }}
                                            </button>
                                        </div>
                                    }
                                })
                                .collect_view()
                                .into_any()
                        }}
                    </div>
                </div>
            </Show>

            <button
                class="panel-expand"
                title="展开"
                on:click=move |_| collapsed.set(false)
            >
                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
                    <path d="M15 6l-6 6 6 6" />
                </svg>
            </button>

            <div class="panel-body">
                {move || {
                    if !configured.get() {
                        view! { <p class="panel-hint">"未配置模型 · 前往设置"</p> }.into_any()
                    } else if turns.get().is_empty() && answer.get().is_empty() && !streaming.get() {
                        view! { <p class="panel-hint">"选中文字加入引用，然后提问"</p> }.into_any()
                    } else {
                        view! {
                            <div
                                id="transcript"
                                class="transcript"
                                on:click=move |event: web_sys::MouseEvent| {
                                    let Some(target) = event
                                        .target()
                                        .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                                    else {
                                        return;
                                    };
                                    if let Some(url) = target.get_attribute("data-url") {
                                        event.prevent_default();
                                        let _ = crate::call("openUrl", &[JsValue::from_str(&url)]);
                                        return;
                                    }
                                    if let Some(number) = target
                                        .get_attribute("data-page")
                                        .and_then(|value| value.parse::<u32>().ok())
                                    {
                                        event.prevent_default();
                                        jump(number);
                                    }
                                }
                            >
                                {move || {
                                    if !(failed.get() && !streaming.get()) {
                                        return ().into_any();
                                    }
                                    view! {
                                        <div class="error-card">
                                            <span>{status.get()}</span>
                                            <button
                                                class="action"
                                                on:click=move |_| {
                                                    if let Some(last) = turns.get().last().cloned() {
                                                        send_text(last.question);
                                                    }
                                                }
                                            >
                                                "重试"
                                            </button>
                                        </div>
                                    }
                                        .into_any()
                                }}

                                {move || {
                                    let all = turns.get();
                                    let last = all.len().saturating_sub(1);
                                    all
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, turn)| {
                                            let detail = if turn.reasoning.is_empty() {
                                                ().into_any()
                                            } else {
                                                view! {
                                                    <details class="think">
                                                        <summary>
                                                            {format!("思考 {:.1} 秒", turn.seconds.max(0.1))}
                                                        </summary>
                                                        <pre class="think-text">{turn.reasoning.clone()}</pre>
                                                    </details>
                                                }
                                                    .into_any()
                                            };

                                            let body = if turn.answer.trim().is_empty() {
                                                view! {
                                                    <p class="turn-empty">
                                                        "模型只返回了思考过程，没有正文。"
                                                    </p>
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <div
                                                        id=format!("answer-{index}")
                                                        class="answer-text markdown"
                                                        inner_html=link_source_markers(
                                                            &pdfscope_core::to_html(&turn.answer),
                                                            &turn
                                                                .sources
                                                                .iter()
                                                                .map(|source| (source.title.clone(), source.url.clone()))
                                                                .collect::<Vec<_>>(),
                                                        )
                                                    ></div>
                                                }
                                                    .into_any()
                                            };

                                            let copied = turn.answer.clone();
                                            let retry = turn.question.clone();
                                            let locate = segments(&turn.answer)
                                                .into_iter()
                                                .find_map(|segment| match segment {
                                                    Segment::Citation(target) => Some(target),
                                                    Segment::Text(_) => None,
                                                });
                                            let located = locate
                                                .map(|target| {
                                                    view! {
                                                        <button
                                                            class="action"
                                                            on:click=move |_| jump(target)
                                                        >
                                                            "定位"
                                                        </button>
                                                    }
                                                });
                                            let regenerate = (index == last)
                                                .then(|| {
                                                    view! {
                                                        <button
                                                            class="action"
                                                            on:click=move |_| send_text(retry.clone())
                                                        >
                                                            "重新生成"
                                                        </button>
                                                    }
                                                });
                                            let continued = (index == last && turn.interrupted)
                                                .then(|| {
                                                    view! {
                                                        <button
                                                            class="action"
                                                            on:click=move |_| continue_last()
                                                        >
                                                            "继续生成"
                                                        </button>
                                                    }
                                                });

                                            let edit_text = turn
                                                .question
                                                .split("\n\n[引用的先前回答]")
                                                .next()
                                                .unwrap_or_default()
                                                .to_owned();
                                            let edit = view! {
                                                <button
                                                    class="action"
                                                    on:click=move |_| {
                                                        if streaming.get() {
                                                            return;
                                                        }
                                                        editing.set(Some(index));
                                                        let _ = composer(
                                                            "setPlainText",
                                                            &[JsValue::from_str(&edit_text)],
                                                        );
                                                    }
                                                >
                                                    "编辑"
                                                </button>
                                            };
                                            let branch = view! {
                                                <button
                                                    class="action"
                                                    title="从这里开始一条新对话"
                                                    on:click=move |_| branch_conversation(index)
                                                >
                                                    "分支"
                                                </button>
                                            };

                                            let answer_sources = (!turn.sources.is_empty()).then(|| {
                                                let sources = turn.sources.clone();
                                                view! {
                                                    <div class="answer-sources">
                                                        {sources
                                                            .into_iter()
                                                            .enumerate()
                                                            .map(|(index, source)| {
                                                                let url = source.url.clone();
                                                                view! {
                                                                    <button
                                                                        class="source-chip"
                                                                        title=source.url.clone()
                                                                        on:click=move |_| {
                                                                            let _ = crate::call(
                                                                                "openUrl",
                                                                                &[JsValue::from_str(&url)],
                                                                            );
                                                                        }
                                                                    >
                                                                        <span class="source-index">{index + 1}</span>
                                                                        <span class="source-title">{source.title.clone()}</span>
                                                                    </button>
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </div>
                                                }
                                            });

                                            let answer_for_copy_cite = turn.answer.clone();
                                            let copy_with_citations = view! {
                                                <button
                                                    class="action"
                                                    title="复制回答并附上引用页码"
                                                    on:click=move |_| {
                                                        let mut pages: Vec<u32> = segments(&answer_for_copy_cite)
                                                            .into_iter()
                                                            .filter_map(|segment| match segment {
                                                                Segment::Citation(page) => Some(page),
                                                                Segment::Text(_) => None,
                                                            })
                                                            .collect();
                                                        pages.sort_unstable();
                                                        pages.dedup();
                                                        let mut text = answer_for_copy_cite.clone();
                                                        if !pages.is_empty() {
                                                            text.push_str("\n\n参考：");
                                                            text.push_str(
                                                                &pages
                                                                    .iter()
                                                                    .map(|page| format!("p.{page}"))
                                                                    .collect::<Vec<_>>()
                                                                    .join("、"),
                                                            );
                                                        }
                                                        copy_to_clipboard(&text);
                                                        notify.run("已复制（含页码）".to_owned());
                                                    }
                                                >
                                                    "复制引用"
                                                </button>
                                            };

                                            let answer_for_cite = turn.answer.clone();
                                            let cite = view! {
                                                <button
                                                    class="action"
                                                    on:click=move |_| {
                                                        if answer_for_cite.trim().is_empty() {
                                                            return;
                                                        }
                                                        undo_push.run(());
                                                        let id = next_answer.get() + 1;
                                                        next_answer.set(id);
                                                        answers.update(|list| {
                                                            list.push(crate::AnswerRef {
                                                                id,
                                                                label: format!("回答 {id}"),
                                                                quote: answer_for_cite.clone(),
                                                            });
                                                        });
                                                        notify.run("已引用这条回答".to_owned());
                                                    }
                                                >
                                                    "引用此回答"
                                                </button>
                                            };

                                            view! {
                                                <div class="turn">
                                                    <p class="turn-question">{turn.question.clone()}</p>
                                                    {detail}
                                                    {body}
                                                    {answer_sources}
                                                    <div class="turn-actions">
                                                        {located}
                                                        {edit}
                                                        {branch}
                                                        {cite}
                                                        <button
                                                            class="action"
                                                            on:click=move |_| {
                                                                copy_to_clipboard(&copied);
                                                                notify.run("已复制".to_owned());
                                                            }
                                                        >
                                                            "复制"
                                                        </button>
                                                        {copy_with_citations}
                                                        {regenerate}
                                                        {continued}
                                                    </div>
                                                </div>
                                            }
                                        })
                                        .collect_view()
                                }}

                                {move || {
                                    if answer.get().is_empty() && !streaming.get() {
                                        return ().into_any();
                                    }
                                    let streaming_now = streaming.get();
                                    let detail = if reasoning.get().is_empty() {
                                        ().into_any()
                                    } else {
                                        let label = if streaming_now {
                                            "思考中…".to_owned()
                                        } else {
                                            format!("思考 {:.1} 秒", think_seconds.get())
                                        };
                                        if streaming_now {
                                            view! {
                                                <div class="think">
                                                    <div class="think-head">{label}</div>
                                                    <pre class="think-text">{reasoning.get()}</pre>
                                                </div>
                                            }
                                                .into_any()
                                        } else {
                                            view! {
                                                <details class="think">
                                                    <summary>{label}</summary>
                                                    <pre class="think-text">{reasoning.get()}</pre>
                                                </details>
                                            }
                                                .into_any()
                                        }
                                    };
                                    view! {
                                        <div class="turn current">
                                            <p class="turn-question">
                                                {move || {
                                                    if continue_target.get().is_some() {
                                                        "继续生成…".to_owned()
                                                    } else {
                                                        current_question.get()
                                                    }
                                                }}
                                            </p>
                                            {detail}
                                            <div
                                                class="answer-text markdown"
                                                aria-live="polite"
                                                inner_html=move || pdfscope_core::to_html(&answer.get())
                                            ></div>
                                            <div class="turn-actions">
                                                <button
                                                    class="action"
                                                    on:click=move |_| {
                                                    copy_to_clipboard(&answer.get());
                                                    notify.run("已复制".to_owned());
                                                }
                                                >
                                                    "复制"
                                                </button>
                                            </div>
                                        </div>
                                    }
                                        .into_any()
                                }}
                            </div>
                        }
                            .into_any()
                    }
                }}
            </div>

            <div class="composer">
                {move || {
                    let all = history.get();
                    if all.is_empty() {
                        return ().into_any();
                    }
                    let start = if show_all.get() { 0 } else { all.len().saturating_sub(3) };
                    let entries: Vec<(usize, Stored)> = all
                        .iter()
                        .cloned()
                        .enumerate()
                        .skip(start)
                        .collect();
                    view! {
                        <div class="history">
                            <div class="history-head">
                                <span>"历史引用"</span>
                                <button
                                    class="link"
                                    on:click=move |_| show_all.update(|open| *open = !*open)
                                >
                                    {move || if show_all.get() { "收起" } else { "全部" }}
                                </button>
                            </div>
                            {entries
                                .into_iter()
                                .map(|(index, entry)| {
                                    let (label, preview, page_number, rect) = match &entry {
                                        Stored::Passage(passage) => (
                                            format!("p.{} · {}", passage.page, truncate(&passage.quote, 14)),
                                            passage.quote.clone(),
                                            passage.page,
                                            passage.rect,
                                        ),
                                        Stored::Region(region) => (
                                            format!("p.{} · 截图", region.page),
                                            "（截图）".to_owned(),
                                            region.page,
                                            region.rect,
                                        ),
                                    };
                                    view! {
                                        <div
                                            class="history-row"
                                            on:mouseenter=move |_| {
                                                hovered.set(Some((page_number, rect)));
                                            }
                                            on:mouseleave=move |_| hovered.set(None)
                                            on:click=move |event: web_sys::MouseEvent| {
                                                if event.shift_key() {
                                                    jump(page_number);
                                                } else {
                                                    redeem(index);
                                                }
                                            }
                                        >
                                            <span class="history-label">{label}</span>
                                            <span class="history-preview">{preview}</span>
                                            <span class="pill">{format!("p.{page_number} →")}</span>
                                        </div>
                                    }
                                })
                                .collect_view()}
                        </div>
                    }
                        .into_any()
                }}
                {move || {
                    let quotes = passages.get();
                    let shots = regions.get();
                    let pages_selected = page_set.get();
                    let cited = answers.get();
                    let pictures = images.get();
                    let documents = files.get();
                    let citations = sources.get();
                    if quotes.is_empty()
                        && shots.is_empty()
                        && pictures.is_empty()
                        && documents.is_empty()
                        && citations.is_empty()
                        && pages_selected.is_empty()
                        && cited.is_empty()
                    {
                        return ().into_any();
                    }

                    let quote_row = (!quotes.is_empty()).then(|| {
                        view! {
                            <div class="shelf-row">
                                <span class="shelf-label">"文字"</span>
                                <div class="shelf-track">
                                    {quotes
                                        .into_iter()
                                        .map(|passage| {
                                            let page_number = passage.page;
                                            let id = passage.id;
                                            view! {
                                                <div
                                                    class="attach-chip"
                                                    draggable="true"
                                                    title=passage.quote.clone()
                                                    on:click=move |_| jump(page_number)
                                                    on:dragstart=move |event: web_sys::DragEvent| {
                                                        if let Some(data) = event.data_transfer() {
                                                            let _ = data.set_data(
                                                                "text/plain",
                                                                &format!("p:{}\np.{}", id, page_number),
                                                            );
                                                            data.set_effect_allowed("copy");
                                                        }
                                                    }
                                                >
                                                    <span class="chip-kind">"文"</span>
                                                    <span class="chip-text">{truncate(&passage.quote, 36)}</span>
                                                    <span class="chip-page">{format!("p.{page_number}")}</span>
                                                    <span
                                                        class="chip-x"
                                                        title="移除"
                                                        on:click=move |event: web_sys::MouseEvent| {
                                                            event.stop_propagation();
                                                            remove_passage(id);
                                                        }
                                                    >
                                                        "✕"
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    });

                    let shot_row = (!shots.is_empty()).then(|| {
                        view! {
                            <div class="shelf-row">
                                <span class="shelf-label">"截图"</span>
                                <div class="shelf-track">
                                    {shots
                                        .into_iter()
                                        .map(|region| {
                                            let page_number = region.page;
                                            let id = region.id;
                                            view! {
                                                <div
                                                    class="attach-thumb"
                                                    draggable="true"
                                                    title=format!("p.{} · 截图", region.page)
                                                    on:click=move |_| jump(page_number)
                                                    on:dragstart=move |event: web_sys::DragEvent| {
                                                        if let Some(data) = event.data_transfer() {
                                                            let _ = data.set_data(
                                                                "text/plain",
                                                                &format!("r:{}\n图 p.{}", id, page_number),
                                                            );
                                                            data.set_effect_allowed("copy");
                                                        }
                                                    }
                                                >
                                                    <img src=region.image.clone() alt="" />
                                                    <span
                                                        class="chip-x thumb-x"
                                                        title="移除"
                                                        on:click=move |event: web_sys::MouseEvent| {
                                                            event.stop_propagation();
                                                            remove_region(id);
                                                        }
                                                    >
                                                        "✕"
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    });

                    let image_row = (!pictures.is_empty()).then(|| {
                        view! {
                            <div class="shelf-row">
                                <span class="shelf-label">"图片"</span>
                                <div class="shelf-track">
                                    {pictures
                                        .into_iter()
                                        .map(|picture| {
                                            let id = picture.id;
                                            view! {
                                                <div class="attach-thumb" title=picture.name.clone()>
                                                    <img src=picture.image.clone() alt="" />
                                                    <span
                                                        class="chip-x thumb-x"
                                                        title="移除"
                                                        on:click=move |event: web_sys::MouseEvent| {
                                                            event.stop_propagation();
                                                            remove_image(id);
                                                        }
                                                    >
                                                        "✕"
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    });

                    let file_row = (!documents.is_empty()).then(|| {
                        view! {
                            <div class="shelf-row">
                                <span class="shelf-label">"文件"</span>
                                <div class="shelf-track">
                                    {documents
                                        .into_iter()
                                        .map(|file| {
                                            let id = file.id;
                                            view! {
                                                <div class="attach-chip" title=file.name.clone()>
                                                    <span class="chip-kind">"件"</span>
                                                    <span class="chip-text">{file.name.clone()}</span>
                                                    <span class="chip-page">
                                                        {format!("{} 字", file.text.chars().count())}
                                                    </span>
                                                    <span
                                                        class="chip-x"
                                                        title="移除"
                                                        on:click=move |event: web_sys::MouseEvent| {
                                                            event.stop_propagation();
                                                            remove_file(id);
                                                        }
                                                    >
                                                        "✕"
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    });

                    let page_row = (!pages_selected.is_empty()).then(|| {
                        view! {
                            <div class="shelf-row">
                                <span class="shelf-label">"页面"</span>
                                <div class="shelf-track">
                                    {pages_selected
                                        .into_iter()
                                        .map(|page_number| {
                                            view! {
                                                <div
                                                    class="attach-chip"
                                                    draggable="true"
                                                    on:click=move |_| jump(page_number)
                                                    on:dragstart=move |event: web_sys::DragEvent| {
                                                        if let Some(data) = event.data_transfer() {
                                                            let _ = data.set_data(
                                                                "text/plain",
                                                                &format!("page:{}\np.{}", page_number, page_number),
                                                            );
                                                            data.set_effect_allowed("copy");
                                                        }
                                                    }
                                                >
                                                    <span class="chip-kind">"页"</span>
                                                    <span class="chip-page">{format!("p.{page_number}")}</span>
                                                    <span
                                                        class="chip-x"
                                                        title="移除"
                                                        on:click=move |event: web_sys::MouseEvent| {
                                                            event.stop_propagation();
                                                            remove_page(page_number);
                                                        }
                                                    >
                                                        "✕"
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    });

                    let source_row = (!citations.is_empty()).then(|| {
                        view! {
                            <div class="shelf-row">
                                <span class="shelf-label">"来源"</span>
                                <div class="shelf-track">
                                    {citations
                                        .into_iter()
                                        .enumerate()
                                        .map(|(index, source)| {
                                            let url = source.url.clone();
                                            view! {
                                                <div
                                                    class="attach-chip"
                                                    title=source.url.clone()
                                                    on:click=move |_| {
                                                        let _ = crate::call(
                                                            "openUrl",
                                                            &[JsValue::from_str(&url)],
                                                        );
                                                    }
                                                >
                                                    <span class="chip-kind">{format!("{}", index + 1)}</span>
                                                    <span class="chip-text">{source.title.clone()}</span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    });

                    let answer_row = (!cited.is_empty()).then(|| {
                        view! {
                            <div class="shelf-row">
                                <span class="shelf-label">"回答"</span>
                                <div class="shelf-track">
                                    {cited
                                        .into_iter()
                                        .map(|answer| {
                                            let id = answer.id;
                                            view! {
                                                <div
                                                    class="attach-chip"
                                                    draggable="true"
                                                    title=answer.quote.clone()
                                                    on:dragstart=move |event: web_sys::DragEvent| {
                                                        if let Some(data) = event.data_transfer() {
                                                            let _ = data.set_data(
                                                                "text/plain",
                                                                &format!("a:{}\n{}", id, answer.label),
                                                            );
                                                            data.set_effect_allowed("copy");
                                                        }
                                                    }
                                                >
                                                    <span class="chip-kind">"✦"</span>
                                                    <span class="chip-text">{truncate(&answer.quote, 32)}</span>
                                                    <span
                                                        class="chip-x"
                                                        title="移除"
                                                        on:click=move |event: web_sys::MouseEvent| {
                                                            event.stop_propagation();
                                                            remove_answer(id);
                                                        }
                                                    >
                                                        "✕"
                                                    </span>
                                                </div>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                            </div>
                        }
                    });

                    view! {
                        <div class="context-shelf">
                            {quote_row}
                            {shot_row}
                            {image_row}
                            {file_row}
                            {source_row}
                            {page_row}
                            {answer_row}
                        </div>
                    }
                        .into_any()
                }}
                <div class="scope-row">
                    <div class="scope-icons" role="group" aria-label="提问范围">
                        <button
                            class="scope-icon"
                            class:active=move || scope_mode.get() == 0
                            title=move || format!("整份文档 · {} 页", pages.get())
                            aria-label="整份文档"
                            aria-pressed=move || (scope_mode.get() == 0).to_string()
                            on:click=move |_| {
                                undo_push.run(());
                                scope_mode.set(0);
                            }
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                                <rect x="7" y="3.5" width="10" height="14" rx="1.6" />
                                <path d="M10 20.5h7a2 2 0 0 0 2-2V8" />
                            </svg>
                        </button>
                        <button
                            class="scope-icon"
                            class:active=move || scope_mode.get() == 1
                            title=move || format!("仅当前页 · p.{}", page.get())
                            aria-label="仅当前页"
                            aria-pressed=move || (scope_mode.get() == 1).to_string()
                            on:click=move |_| {
                                undo_push.run(());
                                scope_mode.set(1);
                            }
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M7 3.5h6l4 4v13H7z" />
                                <path d="M13 3.5v4h4" />
                            </svg>
                        </button>
                        <button
                            class="scope-icon"
                            class:active=move || scope_mode.get() == 2
                            title=move || format!("仅引用 · {} 项", reference_count())
                            aria-label="仅引用"
                            aria-pressed=move || (scope_mode.get() == 2).to_string()
                            on:click=move |_| {
                                undo_push.run(());
                                scope_mode.set(2);
                            }
                        >
                            <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">
                                <path d="M4 20.5h16" />
                                <path d="M7.5 16.5l8-9 3 3-8 9h-3z" />
                                <path d="M12.5 6.5l3 3" />
                            </svg>
                            <span class="scope-badge">
                                {move || {
                                    let count = reference_count();
                                    if count == 0 { String::new() } else { count.to_string() }
                                }}
                            </span>
                        </button>
                    </div>
                    <button class="button scope-manage" on:click=move |_| manager_open.set(true)>
                        "管理页面引用"
                    </button>
                </div>
                {move || {
                    notice
                        .get()
                        .map(|message| view! { <p class="range-summary range-error">{message}</p> })
                }}
                <Show when=move || editing.get().is_some()>
                    <div class="editing-banner">
                        <span>
                            {move || {
                                format!(
                                    "正在编辑第 {} 条问题 · 发送后替换其后的回答",
                                    editing.get().unwrap_or(0) + 1
                                )
                            }}
                        </span>
                        <button
                            class="link"
                            on:click=move |_| {
                                editing.set(None);
                                composer_clear();
                            }
                        >
                            "取消"
                        </button>
                    </div>
                </Show>
                {move || {
                    if !mention_open.get() {
                        return ().into_any();
                    }
                    let mut items = Vec::new();
                    for passage in passages.get() {
                        let key = format!("p:{}", passage.id);
                        let label = format!("p.{}", passage.page);
                        items.push(view! {
                            <button
                                class="mention-item"
                                on:click=move |_| {
                                    let _ = composer(
                                        "insertToken",
                                        &[JsValue::from_str(&key), JsValue::from_str(&label)],
                                    );
                                    mention_open.set(false);
                                }
                            >
                                <span class="mention-kind">"文"</span>
                                <span class="mention-label">{format!("p.{} · {}", passage.page, truncate(&passage.quote, 30))}</span>
                            </button>
                        }.into_any());
                    }
                    for region in regions.get() {
                        let key = format!("r:{}", region.id);
                        let label = format!("图 p.{}", region.page);
                        items.push(view! {
                            <button
                                class="mention-item"
                                on:click=move |_| {
                                    let _ = composer(
                                        "insertToken",
                                        &[JsValue::from_str(&key), JsValue::from_str(&label)],
                                    );
                                    mention_open.set(false);
                                }
                            >
                                <span class="mention-kind">"图"</span>
                                <span class="mention-label">{format!("p.{} · 截图", region.page)}</span>
                            </button>
                        }.into_any());
                    }
                    for page in page_set.get() {
                        let key = format!("page:{page}");
                        let label = format!("p.{page}");
                        items.push(view! {
                            <button
                                class="mention-item"
                                on:click=move |_| {
                                    let _ = composer(
                                        "insertToken",
                                        &[JsValue::from_str(&key), JsValue::from_str(&label)],
                                    );
                                    mention_open.set(false);
                                }
                            >
                                <span class="mention-kind">"页"</span>
                                <span class="mention-label">{format!("p.{page}")}</span>
                            </button>
                        }.into_any());
                    }
                    for file in files.get() {
                        let key = format!("file:{}", file.id);
                        let label = truncate(&file.name, 14);
                        items.push(view! {
                            <button
                                class="mention-item"
                                on:click=move |_| {
                                    let _ = composer(
                                        "insertToken",
                                        &[JsValue::from_str(&key), JsValue::from_str(&label)],
                                    );
                                    mention_open.set(false);
                                }
                            >
                                <span class="mention-kind">"件"</span>
                                <span class="mention-label">{file.name.clone()}</span>
                            </button>
                        }.into_any());
                    }
                    for picture in images.get() {
                        let key = format!("img:{}", picture.id);
                        let label = truncate(&picture.name, 14);
                        items.push(view! {
                            <button
                                class="mention-item"
                                on:click=move |_| {
                                    let _ = composer(
                                        "insertToken",
                                        &[JsValue::from_str(&key), JsValue::from_str(&label)],
                                    );
                                    mention_open.set(false);
                                }
                            >
                                <span class="mention-kind">"图"</span>
                                <span class="mention-label">{format!("{} · 附件图片", picture.name)}</span>
                            </button>
                        }.into_any());
                    }
                    for answer in answers.get() {
                        let key = format!("a:{}", answer.id);
                        let label = answer.label.clone();
                        items.push(view! {
                            <button
                                class="mention-item"
                                on:click=move |_| {
                                    let _ = composer(
                                        "insertToken",
                                        &[JsValue::from_str(&key), JsValue::from_str(&label)],
                                    );
                                    mention_open.set(false);
                                }
                            >
                                <span class="mention-kind">"✦"</span>
                                <span class="mention-label">{format!("{} · {}", answer.label, truncate(&answer.quote, 28))}</span>
                            </button>
                        }.into_any());
                    }
                    let current_page = page.get();
                    let already = page_set.get().contains(&current_page);
                    items.push(view! {
                        <button
                            class="mention-item"
                            on:click=move |_| {
                                if !already {
                                    undo_push.run(());
                                    page_set.update(|set| pdfscope_core::add_range(set, current_page, current_page));
                                    scope_mode.set(2);
                                }
                                let _ = composer(
                                    "insertToken",
                                    &[
                                        JsValue::from_str(&format!("page:{current_page}")),
                                        JsValue::from_str(&format!("p.{current_page}")),
                                    ],
                                );
                                mention_open.set(false);
                            }
                        >
                            <span class="mention-kind">"+"</span>
                            <span class="mention-label">{format!("引用当前页 p.{current_page}")}</span>
                        </button>
                    }.into_any());

                    view! {
                        <div
                            class="mention-picker"
                            on:pointerdown=move |event: web_sys::PointerEvent| event.prevent_default()
                        >
                            <p class="mention-title">"插入引用"</p>
                            {items.collect_view()}
                        </div>
                    }
                        .into_any()
                }}
                <div
                    node_ref=editor_ref
                    id="ask-editor"
                    class="ask editor"
                    contenteditable="true"
                    role="textbox"
                    aria-label="问题"
                    data-placeholder="问点什么…"
                    on:dragover=move |event: web_sys::DragEvent| {
                        event.prevent_default();
                        if let Some(data) = event.data_transfer() {
                            data.set_drop_effect("copy");
                        }
                    }
                    on:drop=move |event: web_sys::DragEvent| {
                        event.prevent_default();
                        let Some(data) = event.data_transfer() else {
                            return;
                        };
                        let Ok(payload) = data.get_data("text/plain") else {
                            return;
                        };
                        let mut parts = payload.splitn(2, '\n');
                        let Some(key) = parts.next() else {
                            return;
                        };
                        let label = parts.next().unwrap_or(key);
                        let _ = composer(
                            "insertTokenAt",
                            &[
                                JsValue::from_f64(f64::from(event.client_x())),
                                JsValue::from_f64(f64::from(event.client_y())),
                                JsValue::from_str(key),
                                JsValue::from_str(label),
                            ],
                        );
                    }
                    on:keydown=move |event: web_sys::KeyboardEvent| {
                        if event.is_composing() {
                            return;
                        }
                        if mention_open.get() {
                            match event.key().as_str() {
                                "ArrowDown" => {
                                    event.prevent_default();
                                    let _ = composer("mentionStep", &[JsValue::from_f64(1.0)]);
                                    return;
                                }
                                "ArrowUp" => {
                                    event.prevent_default();
                                    let _ = composer("mentionStep", &[JsValue::from_f64(-1.0)]);
                                    return;
                                }
                                "Enter" => {
                                    event.prevent_default();
                                    let _ = composer("mentionChoose", &[]);
                                    return;
                                }
                                "Escape" => {
                                    event.prevent_default();
                                    mention_open.set(false);
                                    return;
                                }
                                _ => {}
                            }
                        }
                        if !mention_open.get() && !streaming.get() {
                            let at_start = composer("isAtStart", &[])
                                .and_then(|value| value.as_bool())
                                .unwrap_or(false);
                            let at_end = composer("isAtEnd", &[])
                                .and_then(|value| value.as_bool())
                                .unwrap_or(false);
                            if event.key() == "ArrowUp" && at_start {
                                let history = prompt_history.get();
                                if !history.is_empty() {
                                    event.prevent_default();
                                    let next = history_cursor
                                        .get()
                                        .map(|cursor| cursor.saturating_sub(1))
                                        .unwrap_or(history.len() - 1);
                                    history_cursor.set(Some(next));
                                    let _ = composer(
                                        "setPlainText",
                                        &[JsValue::from_str(&history[next])],
                                    );
                                    return;
                                }
                            }
                            if event.key() == "ArrowDown" && at_end {
                                if let Some(cursor) = history_cursor.get() {
                                    let history = prompt_history.get();
                                    event.prevent_default();
                                    if cursor + 1 < history.len() {
                                        history_cursor.set(Some(cursor + 1));
                                        let _ = composer(
                                            "setPlainText",
                                            &[JsValue::from_str(&history[cursor + 1])],
                                        );
                                    } else {
                                        history_cursor.set(None);
                                        let _ = composer(
                                            "setPlainText",
                                            &[JsValue::from_str("")],
                                        );
                                    }
                                    return;
                                }
                            }
                        }
                        let modified = event.meta_key()
                            || event.ctrl_key()
                            || event.alt_key()
                            || event.shift_key();
                        if event.key() == "Enter" && !modified {
                            event.prevent_default();
                            send(());
                        } else if event.key() == "Enter" && event.shift_key() {
                            event.prevent_default();
                            let _ = composer("insertNewline", &[]);
                        } else if event.key() == "Escape" {
                            mention_open.set(false);
                            if streaming.get() {
                                let _ = bridge("stop", &[]);
                            }
                        }
                    }
                ></div>
                <div class="composer-row">
                    <button
                        class="icon-button attach-button"
                        class:active=move || web.get()
                        title="联网搜索（需要 Tavily Key）"
                        on:click=move |_| web.update(|on| *on = !*on)
                    >
                        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                            <circle cx="12" cy="12" r="8.5" />
                            <path d="M3.5 12h17M12 3.5c2.4 2.3 3.6 5.1 3.6 8.5s-1.2 6.2-3.6 8.5c-2.4-2.3-3.6-5.1-3.6-8.5S9.6 5.8 12 3.5z" />
                        </svg>
                    </button>
                    <label class="icon-button attach-button" title="附加图片、PDF、Office 或文本/代码文件">
                        <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M20 11.5l-7.8 7.8a5 5 0 0 1-7.1-7.1l8.2-8.2a3.4 3.4 0 0 1 4.8 4.8l-8.2 8.2a1.8 1.8 0 0 1-2.5-2.5l7.4-7.4" />
                        </svg>
                        <input
                            id="attach-input"
                            class="file-input"
                            type="file"
                            accept="image/*,.pdf,.docx,.pptx,.xlsx,.txt,.md,.markdown,.csv,.tsv,.json,.jsonl,.log,.yaml,.yml,.html,.htm,.xml,.toml,.ini,.rs,.ts,.tsx,.js,.jsx,.py,.java,.kt,.c,.h,.cpp,.hpp,.cs,.go,.rb,.php,.swift,.sh,.sql,.css,.scss,.vue,.svelte,.lua"
                            multiple
                            on:change=move |_| {
                                let Some(promise) = crate::call(
                                    "readAttachments",
                                    &[JsValue::from_str("attach-input")],
                                ) else {
                                    return;
                                };
                                spawn_local(async move {
                                    let Ok(value) = JsFuture::from(js_sys::Promise::from(promise)).await
                                    else {
                                        return;
                                    };
                                    let Ok(list) = value.dyn_into::<js_sys::Array>() else {
                                        return;
                                    };
                                    for entry in list.iter() {
                                        let kind = crate::model::read_string(&entry, "kind")
                                            .unwrap_or_default();
                                        let name = crate::model::read_string(&entry, "name")
                                            .unwrap_or_else(|| "附件".to_owned());
                                        let data = crate::model::read_string(&entry, "data")
                                            .unwrap_or_default();
                                        match kind.as_str() {
                                            "image" if data.starts_with("data:image/") => {
                                                let id = next_image.get() + 1;
                                                next_image.set(id);
                                                images.update(|list| {
                                                    list.push(UserImage { id, name, image: data })
                                                });
                                            }
                                            "text" if !data.trim().is_empty() => {
                                                let id = next_file.get() + 1;
                                                next_file.set(id);
                                                files.update(|list| {
                                                    list.push(UserFile { id, name, text: data })
                                                });
                                            }
                                            "error" => {
                                                if let Some(message) = crate::model::read_string(
                                                    &entry,
                                                    "message",
                                                ) {
                                                    notify.run(message);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                });
                            }
                        />
                    </label>
                    <span class="status">{move || status.get()}</span>
                    <span class="token-meter" title="粗略估算，真实用量由模型决定">
                        {move || format!("约 {} tokens", token_estimate())}
                    </span>
                    <button
                        class="button button-primary"
                        on:click=move |_| {
                            if streaming.get() {
                                let _ = bridge("stop", &[]);
                            } else {
                                send(());
                            }
                        }
                    >
                        {move || {
                            if streaming.get() {
                                "停止"
                            } else if failed.get() {
                                "重试"
                            } else {
                                "发送"
                            }
                        }}
                    </button>
                </div>
            </div>

            <PageManager
                open=manager_open
                pages=pages
                page=page
                page_set=page_set
                scope_mode=scope_mode
                undo_push=undo_push
            />

            <SettingsSheet
                open=open
                configured=configured
                notify=notify
                model=model
                page_limit=page_limit
                context_limit=context_limit
                model_choices=model_choices
            />

        </aside>
    }
}
