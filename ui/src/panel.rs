//! The AI panel: settings, the question box, and the streamed answer.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;

use pdfscope_core::{Segment, estimate_tokens, link_source_markers, segments, transcript_markdown};

use crate::icons::{icon, paperclip, provider_tile};
use crate::scope_ui::PageManager;
use crate::settings::SettingsSheet;
use crate::{Passage, Region, truncate};

/// Consecutive page numbers collapsed into inclusive ranges.
fn page_ranges(pages: &[u32]) -> Vec<(u32, u32)> {
    let mut ranges: Vec<(u32, u32)> = Vec::new();
    for &page in pages {
        match ranges.last_mut() {
            Some((_, end)) if *end + 1 == page => *end = page,
            _ => ranges.push((page, page)),
        }
    }
    ranges
}

/// A compact icon action with a tooltip and an accessible name.
fn icon_action(
    name: &'static str,
    label: &'static str,
    on_click: impl Fn(web_sys::MouseEvent) + 'static,
) -> impl IntoView {
    view! {
        <button class="icon-action" title=label aria-label=label on:click=on_click>
            {icon(name, 15)}
        </button>
    }
}

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
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
struct TurnContext {
    #[serde(default)]
    passages: Vec<Passage>,
    #[serde(default)]
    regions: Vec<Region>,
    #[serde(default)]
    pages: Vec<u32>,
    #[serde(default)]
    answers: Vec<crate::AnswerRef>,
    #[serde(default)]
    images: Vec<UserImage>,
    #[serde(default)]
    files: Vec<UserFile>,
    #[serde(default)]
    web: bool,
    #[serde(default)]
    scope_mode: u8,
    #[serde(default)]
    page: u32,
}

impl TurnContext {
    fn count(&self) -> usize {
        self.passages.len()
            + self.regions.len()
            + self.pages.len()
            + self.answers.len()
            + self.images.len()
            + self.files.len()
            + usize::from(self.web)
    }

    fn labels(&self) -> Vec<String> {
        let mut labels = Vec::new();
        labels.extend(
            self.passages
                .iter()
                .map(|passage| format!("p.{}", passage.page)),
        );
        labels.extend(
            self.regions
                .iter()
                .map(|region| format!("图 p.{}", region.page)),
        );
        let mut pages = self.pages.clone();
        pages.sort_unstable();
        pages.dedup();
        let mut index = 0;
        while index < pages.len() {
            let start = pages[index];
            let mut end = start;
            while index + 1 < pages.len() && pages[index + 1] == end + 1 {
                index += 1;
                end = pages[index];
            }
            labels.push(if start == end {
                format!("p.{start}")
            } else {
                format!("p.{start}–{end}")
            });
            index += 1;
        }
        labels.extend(self.answers.iter().map(|answer| answer.label.clone()));
        labels.extend(
            self.images
                .iter()
                .map(|image| format!("图片 {}", image.name)),
        );
        labels.extend(self.files.iter().map(|file| format!("文件 {}", file.name)));
        if self.web {
            labels.push("联网搜索".to_owned());
        }
        labels
    }
}

/// A paperclip/count capsule to the left of a question bubble, with the full list in the title.
fn reference_badge(context: &TurnContext) -> Option<AnyView> {
    let count = context.count();
    if count == 0 {
        return None;
    }
    let labels = context.labels();
    let title = format!("{count} 项引用：{}", labels.join(" · "));
    Some(
        view! {
            <button
                type="button"
                class="question-refs-badge"
                title=title.clone()
                aria-label=title
            >
                {paperclip()}
                <span>{count}</span>
            </button>
        }
        .into_any(),
    )
}

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
    /// The model that produced this answer, when known.
    #[serde(default)]
    model: Option<String>,
    /// Everything that was attached to this question, kept for the reference summary and edit-resend.
    #[serde(default)]
    context: TurnContext,
}

/// A half-finished answer saved while streaming, used for crash recovery.
#[derive(serde::Deserialize)]
struct PartialTurn {
    question: String,
    answer: String,
    reasoning: String,
    seconds: f64,
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
    /// Conversation tree. Older files have no tree and are converted from `turns`.
    #[serde(default)]
    tree: ConversationTree,
}

/// One node in a conversation tree: a question and its answer.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct TurnNode {
    id: u64,
    parent: Option<u64>,
    #[serde(default)]
    active_child: Option<u64>,
    turn: Turn,
}

#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize, Default)]
struct ConversationTree {
    #[serde(default)]
    nodes: Vec<TurnNode>,
    #[serde(default)]
    active_path: Vec<u64>,
    #[serde(default)]
    next_id: u64,
}

fn tree_from_turns(turns: &[Turn]) -> ConversationTree {
    let mut tree = ConversationTree {
        nodes: Vec::new(),
        active_path: Vec::new(),
        next_id: 1,
    };
    let mut parent = None;
    for turn in turns {
        let id = tree.next_id.max(1);
        tree.next_id = id + 1;
        tree.nodes.push(TurnNode {
            id,
            parent,
            active_child: None,
            turn: turn.clone(),
        });
        if let Some(parent) = parent {
            if let Some(node) = tree.nodes.iter_mut().find(|node| node.id == parent) {
                node.active_child = Some(id);
            }
        }
        tree.active_path.push(id);
        parent = Some(id);
    }
    tree
}

fn tree_path_to(tree: &ConversationTree, id: u64) -> Vec<u64> {
    let mut path = Vec::new();
    let mut current = Some(id);
    while let Some(node_id) = current {
        let Some(node) = tree.nodes.iter().find(|node| node.id == node_id) else {
            break;
        };
        path.push(node.id);
        current = node.parent;
    }
    path.reverse();
    path
}

fn tree_follow(tree: &ConversationTree, id: u64) -> Vec<u64> {
    let mut path = tree_path_to(tree, id);
    while let Some(node) = path
        .last()
        .and_then(|id| tree.nodes.iter().find(|node| node.id == *id))
    {
        let Some(child) = node.active_child else {
            break;
        };
        path.push(child);
    }
    path
}

fn tree_turns(tree: &ConversationTree) -> Vec<Turn> {
    tree.active_path
        .iter()
        .filter_map(|id| tree.nodes.iter().find(|node| node.id == *id))
        .map(|node| node.turn.clone())
        .collect()
}

fn ensure_tree(conversation: &mut Conversation) {
    if conversation.tree.nodes.is_empty() && !conversation.turns.is_empty() {
        conversation.tree = tree_from_turns(&conversation.turns);
    }
    if conversation.tree.nodes.is_empty() {
        conversation.tree.next_id = conversation.tree.next_id.max(1);
        conversation.turns = Vec::new();
        return;
    }
    if conversation.tree.active_path.is_empty() {
        let root = conversation
            .tree
            .nodes
            .iter()
            .find(|node| node.parent.is_none())
            .map(|node| node.id)
            .or_else(|| conversation.tree.nodes.first().map(|node| node.id));
        if let Some(root) = root {
            conversation.tree.active_path = tree_follow(&conversation.tree, root);
        }
    }
    conversation.turns = tree_turns(&conversation.tree);
}

/// A text file the reader attached to a question.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct UserFile {
    id: u64,
    name: String,
    text: String,
}

/// An image the reader attached to a question.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
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
    provider_preset: RwSignal<String>,
    answers: RwSignal<Vec<crate::AnswerRef>>,
    next_answer: RwSignal<u64>,
    next_ref: RwSignal<u64>,
    document_key: RwSignal<String>,
) -> impl IntoView {
    let model = RwSignal::new("未配置".to_owned());
    let answer = RwSignal::new(String::new());
    let turns = RwSignal::new(Vec::<Turn>::new());
    let tree = RwSignal::new(ConversationTree::default());
    let pending_branch = RwSignal::new(Option::<Option<u64>>::None);
    let current_question = RwSignal::new(String::new());
    let current_context = RwSignal::new(TurnContext::default());
    let interrupted = RwSignal::new(false);
    let continue_target = RwSignal::new(None::<usize>);
    let reasoning = RwSignal::new(String::new());
    let reasoning_open = RwSignal::new(true);
    let reasoning_auto_collapsed = RwSignal::new(false);
    let think_started = RwSignal::new(0.0_f64);
    let think_seconds = RwSignal::new(0.0_f64);
    let think_ticker = RwSignal::new(0_i32);
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
    let model_menu = RwSignal::new(false);
    let request_started = RwSignal::new(0_u64);
    let next_file = RwSignal::new(0_u64);
    let editor_ref = NodeRef::<leptos::html::Div>::new();
    let failed = RwSignal::new(false);
    let history = RwSignal::new(Vec::<Stored>::new());
    let show_all = RwSignal::new(false);
    let notice = RwSignal::new(Option::<String>::None);
    let collapsed = RwSignal::new(false);
    let width = RwSignal::new(380_u32);

    // Restore the last width; the divider remembers it per machine.
    Effect::new(move |_| {
        if let Some(width_now) = crate::call("loadPanelWidth", &[]).and_then(|value| value.as_f64())
        {
            if width_now.is_finite() {
                width.set((width_now as u32).clamp(320, 720));
            }
        }
    });
    // JS can finish the drag even when pointerup lands outside the divider.
    Effect::new(move |_| {
        let callback = Closure::<dyn Fn(f64)>::new(move |reported: f64| {
            if reported.is_finite() {
                let reported = reported as u32;
                width.set(reported);
                let _ = crate::call("savePanelWidth", &[JsValue::from_f64(f64::from(reported))]);
            }
        });
        if let Some(window) = web_sys::window() {
            let _ = js_sys::Reflect::set(
                &window,
                &JsValue::from_str("__pdfscopePanelResized"),
                callback.as_ref(),
            );
        }
        callback.forget();
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

    let capture_context = move || TurnContext {
        passages: passages.get_untracked(),
        regions: regions.get_untracked(),
        pages: page_set.get_untracked(),
        answers: answers.get_untracked(),
        images: images.get_untracked(),
        files: files.get_untracked(),
        web: web.get_untracked(),
        scope_mode: scope_mode.get_untracked(),
        page: page.get_untracked(),
    };

    let restore_context = move |context: &TurnContext| {
        passages.set(context.passages.clone());
        regions.set(context.regions.clone());
        page_set.set(context.pages.clone());
        answers.set(context.answers.clone());
        images.set(context.images.clone());
        files.set(context.files.clone());
        web.set(context.web);
        scope_mode.set(context.scope_mode.min(2));
        if context.scope_mode == 1 && context.page > 0 {
            page.set(context.page);
        }
        let max_ref = context
            .passages
            .iter()
            .map(|entry| entry.id)
            .chain(context.regions.iter().map(|entry| entry.id))
            .max()
            .unwrap_or(0);
        next_ref.set(max_ref + 1);
        let max_answer = context
            .answers
            .iter()
            .map(|answer| answer.id)
            .max()
            .unwrap_or(0);
        next_answer.set(max_answer + 1);
        let max_image = context
            .images
            .iter()
            .map(|image| image.id)
            .max()
            .unwrap_or(0);
        next_image.set(max_image + 1);
        let max_file = context.files.iter().map(|file| file.id).max().unwrap_or(0);
        next_file.set(max_file + 1);
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
            if let Some(turn) = turns.get_untracked().get(index).cloned() {
                let mut tree_now = tree.get_untracked();
                if let Some(id) = tree_now.active_path.get(index).copied() {
                    if let Some(node) = tree_now.nodes.iter_mut().find(|node| node.id == id) {
                        node.turn = turn;
                    }
                }
                tree.set(tree_now);
            }
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
            model: session_model.get(),
            context: current_context.get_untracked(),
        };
        let mut tree_now = tree.get_untracked();
        let id = tree_now.next_id.max(1);
        tree_now.next_id = id + 1;
        let parent = match pending_branch.get_untracked() {
            Some(parent) => parent,
            None => tree_now.active_path.last().copied(),
        };
        tree_now.nodes.push(TurnNode {
            id,
            parent,
            active_child: None,
            turn: finished,
        });
        if let Some(parent) = parent {
            if let Some(node) = tree_now.nodes.iter_mut().find(|node| node.id == parent) {
                node.active_child = Some(id);
            }
        }
        let mut path = parent
            .map(|parent| tree_path_to(&tree_now, parent))
            .unwrap_or_default();
        path.push(id);
        tree_now.active_path = path;
        turns.set(tree_turns(&tree_now));
        tree.set(tree_now);
        pending_branch.set(None);
        // The answer keeps its own copy now; the shelf row is only for the live turn.
        sources.set(Vec::new());
        current_question.set(String::new());
        answer.set(String::new());
        reasoning.set(String::new());
    };

    let draft_key_for = move |id: &str| format!("{}.{}", document_key.get_untracked(), id);

    let label_for = move |key: &str| -> Option<String> {
        if let Some(id) = key.strip_prefix("p:").and_then(|id| id.parse::<u64>().ok()) {
            passages
                .get_untracked()
                .iter()
                .find(|passage| passage.id == id)
                .map(|passage| format!("p.{}", passage.page))
        } else if let Some(id) = key.strip_prefix("r:").and_then(|id| id.parse::<u64>().ok()) {
            regions
                .get_untracked()
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
                .get_untracked()
                .iter()
                .find(|answer| answer.id == id)
                .map(|answer| answer.label.clone())
        } else if let Some(id) = key
            .strip_prefix("img:")
            .and_then(|id| id.parse::<u64>().ok())
        {
            images
                .get_untracked()
                .iter()
                .find(|image| image.id == id)
                .map(|image| truncate(&image.name, 14))
        } else if let Some(id) = key
            .strip_prefix("file:")
            .and_then(|id| id.parse::<u64>().ok())
        {
            files
                .get_untracked()
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
        if temporary.get_untracked().is_some() {
            return;
        }
        let id = active.get_untracked();
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
        if temporary.get_untracked().is_some() || id.is_empty() {
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
        let had_markers = text.contains("@{");
        let _ = composer("setRich", &[parts.into()]);
        if had_markers {
            // The editor now holds only the markers whose attachments still exist; persist that
            // cleaned version right away so a restart cannot restore phantom pills.
            let cleaned = question.get_untracked();
            let _ = composer(
                "saveDraftSoon",
                &[JsValue::from_str(&key), JsValue::from_str(&cleaned)],
            );
        }
    };

    let drop_draft = move |id: &str| {
        if id.is_empty() {
            return;
        }
        let key = draft_key_for(id);
        let _ = composer("removeDraft", &[JsValue::from_str(&key)]);
    };

    let partial_key = move || {
        let id = active.get_untracked();
        (!id.is_empty()).then(|| draft_key_for(&id))
    };

    let save_partial_now = move || {
        if temporary.get_untracked().is_some() {
            return;
        }
        let Some(key) = partial_key() else {
            return;
        };
        let answer_now = answer.get();
        let reasoning_now = reasoning.get();
        if answer_now.trim().is_empty() && reasoning_now.trim().is_empty() {
            return;
        }
        let payload = serde_json::json!({
            "question": current_question.get(),
            "answer": answer_now,
            "reasoning": reasoning_now,
            "seconds": think_seconds.get().max(0.1),
        })
        .to_string();
        let _ = composer(
            "savePartialSoon",
            &[JsValue::from_str(&key), JsValue::from_str(&payload)],
        );
    };

    let drop_partial = move || {
        if let Some(key) = partial_key() {
            let _ = composer("clearPartial", &[JsValue::from_str(&key)]);
        }
    };

    let restore_partial = move |id: &str| {
        if temporary.get_untracked().is_some() || id.is_empty() {
            return;
        }
        let key = draft_key_for(id);
        let stored = composer("loadPartial", &[JsValue::from_str(&key)])
            .and_then(|value| value.as_string())
            .unwrap_or_default();
        if stored.trim().is_empty() {
            return;
        }
        let _ = composer("clearPartial", &[JsValue::from_str(&key)]);
        let Ok(partial) = serde_json::from_str::<PartialTurn>(&stored) else {
            return;
        };
        if partial.answer.trim().is_empty() && partial.reasoning.trim().is_empty() {
            return;
        }
        turns.update(|list| {
            list.push(Turn {
                question: partial.question,
                answer: partial.answer,
                reasoning: partial.reasoning,
                seconds: partial.seconds.max(0.1),
                interrupted: true,
                sources: Vec::new(),
                model: None,
                context: TurnContext::default(),
            });
        });
        notify.run("已恢复上次被打断的回答".to_owned());
    };

    // The editor reports text and the attachment keys of its pills.
    Effect::new(move |_| {
        let callback =
            Closure::<dyn Fn(String, JsValue)>::new(move |text: String, keys: JsValue| {
                let current: Vec<String> = keys
                    .dyn_into::<js_sys::Array>()
                    .map(|list| list.iter().filter_map(|value| value.as_string()).collect())
                    .unwrap_or_default();

                let previous = known_mentions.get_untracked();
                for key in previous.iter().filter(|key| !current.contains(key)) {
                    // Remove only what still exists. `update` notifies even when the list is
                    // unchanged, and a restore that re-emits the same keys used to turn that
                    // notification into a reactive loop.
                    if let Some(id) = key.strip_prefix("p:").and_then(|id| id.parse::<u64>().ok()) {
                        if passages.get_untracked().iter().any(|entry| entry.id == id) {
                            undo_push.run(());
                            passages.update(|list| list.retain(|entry| entry.id != id));
                        }
                    } else if let Some(id) =
                        key.strip_prefix("r:").and_then(|id| id.parse::<u64>().ok())
                    {
                        if regions.get_untracked().iter().any(|entry| entry.id == id) {
                            undo_push.run(());
                            regions.update(|list| list.retain(|entry| entry.id != id));
                        }
                    } else if let Some(page) = key
                        .strip_prefix("page:")
                        .and_then(|page| page.parse::<u32>().ok())
                    {
                        if page_set.get_untracked().contains(&page) {
                            undo_push.run(());
                            page_set.update(|set| set.retain(|entry| *entry != page));
                        }
                    } else if let Some(id) =
                        key.strip_prefix("a:").and_then(|id| id.parse::<u64>().ok())
                    {
                        if answers.get_untracked().iter().any(|entry| entry.id == id) {
                            undo_push.run(());
                            answers.update(|list| list.retain(|entry| entry.id != id));
                        }
                    } else if let Some(id) = key
                        .strip_prefix("img:")
                        .and_then(|id| id.parse::<u64>().ok())
                    {
                        if images.get_untracked().iter().any(|entry| entry.id == id) {
                            images.update(|list| list.retain(|entry| entry.id != id));
                        }
                    } else if let Some(id) = key
                        .strip_prefix("file:")
                        .and_then(|id| id.parse::<u64>().ok())
                    {
                        if files.get_untracked().iter().any(|entry| entry.id == id) {
                            files.update(|list| list.retain(|entry| entry.id != id));
                        }
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
        // The document-load Effect calls this when a library is empty. A tracked read here would
        // subscribe that Effect to the counter it is about to write, and it would re-run forever.
        let serial = next_conversation.get_untracked() + 1;
        next_conversation.set(serial);
        format!("{}-{serial}", js_sys::Date::now() as u64)
    };

    let save_conversations = move || {
        let key = document_key.get_untracked();
        if key.trim().is_empty() {
            return;
        }
        // An empty conversation is a scratch pad, not history; only conversations that produced a
        // turn are persisted.
        let stored: Vec<Conversation> = conversations
            .get_untracked()
            .into_iter()
            .filter(|conversation| !conversation.turns.is_empty())
            .collect();
        if let Ok(json) = serde_json::to_string(&stored) {
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
                tree: ConversationTree::default(),
            });
        }

        // Older builds stored empty conversations; they are not history.
        list.retain(|conversation| !conversation.turns.is_empty());
        for conversation in &mut list {
            ensure_tree(conversation);
        }
        if list.is_empty() {
            let id = new_conversation_id();
            list.push(Conversation {
                title: "新对话".to_owned(),
                id,
                updated: js_sys::Date::now(),
                pinned: false,
                model: None,
                turns: Vec::new(),
                tree: ConversationTree::default(),
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
        let first_tree = list[0].tree.clone();
        let first_model = list[0].model.clone();
        conversations.set(list);
        active.set(first.clone());
        tree.set(first_tree);
        turns.set(first_turns);
        session_model.set(first_model);
        pending_branch.set(None);
        editing.set(None);
        restore_draft(&first);
        restore_partial(&first);
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
        let current_tree = tree.get();
        let id = active.get();
        if id.is_empty() || temporary.get().is_some() {
            return;
        }
        conversations.update(|list| {
            if let Some(conversation) = list.iter_mut().find(|entry| entry.id == id) {
                if conversation.turns != current_turns || conversation.tree != current_tree {
                    conversation.updated = js_sys::Date::now();
                    if conversation.title.trim().is_empty() || conversation.title == "新对话" {
                        conversation.title = conversation_title(&current_turns);
                    }
                    conversation.turns = current_turns.clone();
                    conversation.tree = current_tree.clone();
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
        let _ = crate::call("panelSent", &[]);
        answer.set(String::new());
        reasoning.set(String::new());
        reasoning_open.set(true);
        reasoning_auto_collapsed.set(false);
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

            // Trim page context to the configured budget before refusing: references and images are
            // what the reader chose explicitly, pages are bulk that can be shortened.
            let mut base_chars = asked.chars().count();
            for passage in &passages_now {
                base_chars += passage.quote.chars().count();
            }
            let images_count = regions_now.len() + pictures.len();
            let limit = context_limit.get();
            let requested_pages = context.len();
            let mut kept: Vec<(u32, String)> = Vec::new();
            let mut kept_chars = base_chars;

            for (number, text) in context {
                let candidate = kept_chars + text.chars().count();
                if limit > 0 && estimate_tokens(candidate, images_count, 0) > limit as usize {
                    break;
                }
                kept_chars = candidate;
                kept.push((number, text));
            }
            let trimmed = kept.len() < requested_pages;
            context = kept;

            let estimate = estimate_tokens(kept_chars, images_count, 0);
            if limit > 0 && estimate > limit as usize {
                status.set(format!(
                    "引用与附件本身约 {estimate} tokens，超过上限 {limit}；请减少引用或调大上限"
                ));
                failed.set(true);
                streaming.set(false);
                pending_clear.set(false);
                return;
            }
            if trimmed {
                notice.set(Some(format!(
                    "上下文超出上限，已自动发送前 {} 页",
                    context.len()
                )));
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
                        // Thinking is done once answer tokens arrive; the reader may still reopen it.
                        if !reasoning_auto_collapsed.get() && !answer.get().trim().is_empty() {
                            reasoning_open.set(false);
                            reasoning_auto_collapsed.set(true);
                        }
                    }
                    save_partial_now();
                };
                let kind = text_of(&event, "kind").unwrap_or_default();
                match kind.as_str() {
                    "reasoning" => {
                        if let Some(delta) = text_of(&event, "text") {
                            if think_started.get() == 0.0 {
                                think_started.set(js_sys::Date::now());
                            }
                            reasoning.update(|buffer| buffer.push_str(&delta));
                            save_partial_now();
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
                        drop_partial();
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
                        drop_partial();

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
                        drop_partial();
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
        let edit_index = editing.get();
        if let Some(index) = edit_index {
            let tree_now = tree.get_untracked();
            let parent = tree_now
                .active_path
                .get(index)
                .and_then(|id| tree_now.nodes.iter().find(|node| node.id == *id))
                .and_then(|node| node.parent);
            pending_branch.set(Some(parent));
            editing.set(None);
        } else {
            pending_branch.set(None);
        }
        current_context.set(capture_context());
        last_asked.set(text.clone());
        send_text(text);
        // Only a request that really started should clear the field; an early return (not
        // configured, empty question) must leave the draft alone.
        if streaming.get() {
            if let Some(index) = edit_index {
                turns.update(|list| list.truncate(index));
            }
            pending_clear.set(true);
        } else {
            pending_branch.set(None);
        }
    };

    // Recreate one turn as a sibling at its own parent; this works for any node, not just the last.
    let regenerate_at = Callback::new(move |index: usize| {
        if streaming.get() {
            return;
        }
        if !configured.get() {
            open.set(true);
            return;
        }
        let all = turns.get();
        let Some(turn) = all.get(index).cloned() else {
            return;
        };
        let tree_now = tree.get_untracked();
        let parent = tree_now
            .active_path
            .get(index)
            .and_then(|id| tree_now.nodes.iter().find(|node| node.id == *id))
            .and_then(|node| node.parent);
        current_context.set(turn.context.clone());
        pending_branch.set(Some(parent));
        editing.set(None);
        send_text(turn.question);
        if streaming.get() {
            turns.update(|list| list.truncate(index));
        } else {
            pending_branch.set(None);
        }
    });

    // Branch info for one node on the active path: exactly the versions of this question.
    let branch_at = move |index: usize| -> Option<(usize, usize)> {
        if streaming.get() {
            return None;
        }
        let tree_now = tree.get();
        let id = *tree_now.active_path.get(index)?;
        let node = tree_now.nodes.iter().find(|node| node.id == id)?;
        let mut siblings: Vec<u64> = tree_now
            .nodes
            .iter()
            .filter(|entry| entry.parent == node.parent)
            .map(|entry| entry.id)
            .collect();
        siblings.sort_unstable();
        if siblings.len() < 2 {
            return None;
        }
        let position = siblings.iter().position(|entry| *entry == id)?;
        Some((position + 1, siblings.len()))
    };

    let shift_branch_at = Callback::new(move |(index, delta): (usize, i64)| {
        if streaming.get() {
            return;
        }
        let mut tree_now = tree.get_untracked();
        let Some(id) = tree_now.active_path.get(index).copied() else {
            return;
        };
        let Some(node) = tree_now.nodes.iter().find(|node| node.id == id) else {
            return;
        };
        let parent = node.parent;
        let mut siblings: Vec<u64> = tree_now
            .nodes
            .iter()
            .filter(|entry| entry.parent == parent)
            .map(|entry| entry.id)
            .collect();
        siblings.sort_unstable();
        if siblings.len() < 2 {
            return;
        }
        let Some(position) = siblings.iter().position(|entry| *entry == id) else {
            return;
        };
        let next = (position as i64 + delta).rem_euclid(siblings.len() as i64) as usize;
        let target = siblings[next];
        if let Some(parent) = parent {
            if let Some(parent_node) = tree_now.nodes.iter_mut().find(|node| node.id == parent) {
                parent_node.active_child = Some(target);
            }
        }
        tree_now.active_path = tree_follow(&tree_now, target);
        let new_turns = tree_turns(&tree_now);
        session_model.set(new_turns.last().and_then(|turn| turn.model.clone()));
        tree.set(tree_now);
        turns.set(new_turns);
        pending_branch.set(None);
        editing.set(None);
    });

    // The thinking timer ticks while the model is in its reasoning phase and stops at the first
    // answer token; the final duration then stays frozen in the collapsed pill.
    Effect::new(move |_| {
        let running = streaming.get() && answer.get().is_empty();
        let previous = think_ticker.get_untracked();
        if previous != 0 {
            if let Some(window) = web_sys::window() {
                let _ = window.clear_interval_with_handle(previous);
            }
            think_ticker.set(0);
        }
        if !running {
            return;
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let tick = Closure::<dyn Fn()>::new(move || {
            let started = think_started.get_untracked();
            if started > 0.0 {
                think_seconds.set(((js_sys::Date::now() - started) / 1000.0).max(0.1));
            }
        });
        if let Ok(id) = window.set_interval_with_callback_and_timeout_and_arguments_0(
            tick.as_ref().unchecked_ref(),
            100,
        ) {
            think_ticker.set(id);
        }
        tick.forget();
    });

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
                        if let Some(index) = turns.get().len().checked_sub(1) {
                            regenerate_at.run(index);
                        }
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
        conversations.update(|list| list.retain(|entry| !entry.turns.is_empty() || entry.id == id));
        active.set(id.clone());
        session_model.set(conversation.model.clone());
        tree.set(conversation.tree.clone());
        turns.set(conversation.turns);
        pending_branch.set(None);
        temporary.set(None);
        reset_transient();
        restore_draft(&id);
        restore_partial(&id);
        history_open.set(false);
        sync_ready.set(true);
    };

    let create_conversation = move || {
        if streaming.get() {
            return;
        }
        let id = new_conversation_id();
        conversations.update(|list| {
            list.retain(|entry| !entry.turns.is_empty());
            list.push(Conversation {
                id: id.clone(),
                title: "新对话".to_owned(),
                updated: js_sys::Date::now(),
                pinned: false,
                model: None,
                turns: Vec::new(),
                tree: ConversationTree::default(),
            });
        });
        active.set(id.clone());
        session_model.set(None);
        tree.set(ConversationTree::default());
        turns.set(Vec::new());
        pending_branch.set(None);
        temporary.set(None);
        reset_transient();
        restore_draft(&id);
        restore_partial(&id);
        history_open.set(false);
        save_conversations();
    };

    let delete_conversation = move |id: String| {
        if streaming.get() {
            return;
        }
        drop_draft(&id);
        {
            let key = draft_key_for(&id);
            let _ = composer("clearPartial", &[JsValue::from_str(&key)]);
        }
        conversations.update(|list| {
            list.retain(|entry| entry.id != id && !entry.turns.is_empty());
        });
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
                tree.set(next.tree.clone());
                turns.set(next.turns);
                pending_branch.set(None);
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
        tree.set(ConversationTree::default());
        turns.set(Vec::new());
        pending_branch.set(None);
        reset_transient();
        history_open.set(false);
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
                        let _ = crate::call("beginPanelResize", &[]);
                    }
                    on:dblclick=move |_| {
                        width.set(380);
                        let _ = crate::call("savePanelWidth", &[JsValue::from_f64(380.0)]);
                    }
                    on:pointerup=move |_| {
                        let _ = crate::call("finishPanelResize", &[]);
                        let final_width = crate::call("panelWidth", &[])
                            .and_then(|value| value.as_f64())
                            .map(|value| value as u32)
                            .unwrap_or_else(|| width.get());
                        width.set(final_width);
                        let _ = crate::call(
                            "savePanelWidth",
                            &[JsValue::from_f64(f64::from(final_width))],
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
                        class="icon-button panel-toggle"
                        title="收起"
                        on:click=move |_| collapsed.set(true)
                    >
                        {icon("chevron-right", 18)}
                    </button>
                    <span class="provider-slot">
                        {move || provider_tile(provider_preset.get(), 28)}
                    </span>
                    <div class="panel-identity">
                        <span class="panel-title">"AI 助手"</span>
                        <div class="model-anchor">
                            <button
                                class="model-chip"
                                title="选择模型"
                                on:click=move |_| model_menu.update(|open| *open = !*open)
                            >
                                <span class="model-chip-label">
                                    {move || {
                                        session_model.get().unwrap_or_else(|| model.get())
                                    }}
                                </span>
                                {icon("chevron-down", 12)}
                            </button>
                            <Show when=move || model_menu.get()>
                                <div
                                    class="model-backdrop"
                                    on:pointerdown=move |_| model_menu.set(false)
                                ></div>
                                <div class="model-menu">
                                    <div class="model-menu-head">
                                        {provider_tile(provider_preset.get(), 18)}
                                        <span>"选择模型"</span>
                                    </div>
                                    <button
                                        class="model-option"
                                        on:click=move |_| {
                                            session_model.set(None);
                                            let id = active.get();
                                            conversations.update(|list| {
                                                if let Some(conversation) =
                                                    list.iter_mut().find(|entry| entry.id == id)
                                                {
                                                    conversation.model = None;
                                                    conversation.updated = js_sys::Date::now();
                                                }
                                            });
                                            save_conversations();
                                            model_menu.set(false);
                                        }
                                    >
                                        <span class="model-option-icon">
                                            {provider_tile(provider_preset.get(), 18)}
                                        </span>
                                        <span class="model-option-label">
                                            {format!("默认 · {}", model.get())}
                                        </span>
                                        <Show when=move || session_model.get().is_none()>
                                            {icon("check", 14)}
                                        </Show>
                                    </button>
                                    {move || {
                                        let current = session_model.get();
                                        model_choices
                                            .get()
                                            .into_iter()
                                            .map(|choice| {
                                                let selected = current.as_deref() == Some(choice.as_str());
                                                let next = choice.clone();
                                                view! {
                                                    <button
                                                        class="model-option"
                                                        on:click=move |_| {
                                                            let next = Some(next.clone());
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
                                                            model_menu.set(false);
                                                        }
                                                    >
                                                        <span class="model-option-icon">
                                                            {provider_tile(provider_preset.get(), 18)}
                                                        </span>
                                                        <span class="model-option-label">{choice}</span>
                                                        <Show when=move || selected>
                                                            {icon("check", 14)}
                                                        </Show>
                                                    </button>
                                                }
                                            })
                                            .collect_view()
                                    }}
                                    <div class="model-menu-sep"></div>
                                    <button
                                        class="model-menu-foot"
                                        on:click=move |_| {
                                            model_menu.set(false);
                                            open.set(true);
                                        }
                                    >
                                        {icon("settings", 14)}
                                        "管理模型"
                                    </button>
                                </div>
                            </Show>
                        </div>
                    </div>
                    <Show when=move || temporary.get().is_some()>
                        <span class="temp-badge" title="这个对话不会被保存">"临时"</span>
                    </Show>
                    <div class="panel-head-actions">
                        <button
                            class="icon-button"
                            title="历史对话"
                            on:click=move |_| history_open.set(true)
                        >
                            {icon("history", 17)}
                        </button>
                        <button
                            class="icon-button"
                            title="新对话"
                            on:click=move |_| create_conversation()
                        >
                            {icon("plus", 17)}
                        </button>
                    </div>
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
                                {icon("close", 16)}
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
                                list.retain(|conversation| !conversation.turns.is_empty());
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
                                        let copy_turns = conversation.turns.clone();

                                        view! {
                                            <div class="history-row" class:active=is_active>
                                                <button
                                                    class="history-pin"
                                                    class:active=pinned
                                                    title=if pinned { "取消置顶" } else { "置顶" }
                                                    on:click=move |_| toggle_pin(id_for_pin.clone())
                                                >
                                                    {icon("pin", 14)}
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
                                                    title="复制对话"
                                                    on:click=move |_| {
                                                        let pairs: Vec<(String, String)> = copy_turns
                                                            .iter()
                                                            .map(|turn| (turn.question.clone(), turn.answer.clone()))
                                                            .collect();
                                                        let markdown = transcript_markdown(&pairs);
                                                        if markdown.is_empty() {
                                                            return;
                                                        }
                                                        copy_to_clipboard(&markdown);
                                                        notify.run("已复制对话".to_owned());
                                                    }
                                                >
                                                    {icon("copy", 14)}
                                                </button>
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
                                                    {icon("edit", 14)}
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
                                                    {if deleting {
                                                        view! { "确认" }.into_any()
                                                    } else {
                                                        icon("trash", 14).into_any()
                                                    }}
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
                    title="展开 AI 面板"
                    on:click=move |_| collapsed.set(false)
                >
                    {icon("chevron-left", 18)}
                </button>

                <div class="panel-body">
                    {move || {
                        if !configured.get() {
                            view! {
                                <div class="empty-state">
                                    {provider_tile(provider_preset.get(), 40)}
                                    <h3>"先连接一个模型"</h3>
                                    <p>"在设置里填入接口、模型与 Key，然后回来提问。"</p>
                                    <button
                                        class="empty-action"
                                        on:click=move |_| open.set(true)
                                    >
                                        {icon("settings", 14)}
                                        "打开设置"
                                    </button>
                                </div>
                            }
                                .into_any()
                        } else if turns.get().is_empty() && !streaming.get() && current_question.get().is_empty() {
                            view! {
                                <div class="empty-state">
                                    {provider_tile(provider_preset.get(), 40)}
                                    <h3>"从这页开始问"</h3>
                                    <p>"选中文字或框选页面加入引用，再问任何问题。"</p>
                                    <div class="empty-suggestions">
                                        <button
                                            on:click=move |_| {
                                                let _ = composer("setPlainText", &[JsValue::from_str("总结当前页的要点")]);
                                                let _ = composer("focus", &[]);
                                            }
                                        >
                                            "总结本页"
                                        </button>
                                        <button
                                            on:click=move |_| {
                                                let _ = composer("setPlainText", &[JsValue::from_str("解释我选中的段落")]);
                                                let _ = composer("focus", &[]);
                                            }
                                        >
                                            "解释选中段落"
                                        </button>
                                        <button
                                            on:click=move |_| {
                                                let _ = composer("setPlainText", &[JsValue::from_str("对比上一轮回答和当前页")]);
                                                let _ = composer("focus", &[]);
                                            }
                                        >
                                            "和上一轮对比"
                                        </button>
                                    </div>
                                </div>
                            }
                                .into_any()
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
                                            <div class="error-card" role="alert">
                                                <span class="error-icon">{icon("alert", 18)}</span>
                                                <div class="error-copy">
                                                    <strong>"这次没有成功"</strong>
                                                    <span>{status.get()}</span>
                                                </div>
                                                <div class="error-actions">
                                                    {icon_action(
                                                        "retry",
                                                        "重试",
                                                        move |_event: web_sys::MouseEvent| {
                                                            if let Some(index) = turns.get().len().checked_sub(1) {
                                                                regenerate_at.run(index);
                                                            }
                                                        },
                                                    )}
                                                    {icon_action(
                                                        "settings",
                                                        "打开设置",
                                                        move |_event: web_sys::MouseEvent| open.set(true),
                                                    )}
                                                </div>
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
                                                        <details class="think-pill">
                                                            <summary>
                                                                {icon("brain", 14)}
                                                                <span>{format!("思考 {:.1} 秒", turn.seconds.max(0.1))}</span>
                                                                <span class="think-chevron">{icon("chevron-down", 12)}</span>
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
                                                let locate = segments(&turn.answer)
                                                    .into_iter()
                                                    .find_map(|segment| match segment {
                                                        Segment::Citation(target) => Some(target),
                                                        Segment::Text(_) => None,
                                                    });
                                                let located = locate.map(|target| {
                                                    icon_action(
                                                        "page",
                                                        "定位到引用页",
                                                        move |_event: web_sys::MouseEvent| jump(target),
                                                    )
                                                });
                                                let regenerate = icon_action(
                                                    "retry",
                                                    "重新生成这条回答",
                                                    move |_event: web_sys::MouseEvent| {
                                                        regenerate_at.run(index)
                                                    },
                                                );
                                                let continued = (index == last && turn.interrupted).then(|| {
                                                    icon_action(
                                                        "send",
                                                        "继续生成",
                                                        move |_event: web_sys::MouseEvent| continue_last(),
                                                    )
                                                });

                                                let edit_text = turn
                                                    .question
                                                    .split("\n\n[引用的先前回答]")
                                                    .next()
                                                    .unwrap_or_default()
                                                    .to_owned();
                                                let branch_nav = move || {
                                                    branch_at(index).map(|(position, total)| {
                                                        view! {
                                                            <div class="branch-nav" title="切换这个问题的其他版本">
                                                                <button
                                                                    class="branch-step"
                                                                    aria-label="上一个版本"
                                                                    on:click=move |_| shift_branch_at.run((index, -1))
                                                                >
                                                                    {icon("chevron-left", 15)}
                                                                </button>
                                                                <span class="branch-count">
                                                                    {format!("{position} / {total}")}
                                                                </span>
                                                                <button
                                                                    class="branch-step"
                                                                    aria-label="下一个版本"
                                                                    on:click=move |_| shift_branch_at.run((index, 1))
                                                                >
                                                                    {icon("chevron-right", 15)}
                                                                </button>
                                                            </div>
                                                        }
                                                    })
                                                };
                                                let edit_context = turn.context.clone();
                                                let refs = reference_badge(&turn.context);
                                                let edit = icon_action(
                                                    "edit",
                                                    "编辑这个问题并重发",
                                                    move |_event: web_sys::MouseEvent| {
                                                        if streaming.get() {
                                                            return;
                                                        }
                                                        restore_context(&edit_context);
                                                        editing.set(Some(index));
                                                        let _ = composer(
                                                            "setPlainText",
                                                            &[JsValue::from_str(&edit_text)],
                                                        );
                                                        let _ = composer("focus", &[]);
                                                    },
                                                );
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

                                                let answer_for_cite = turn.answer.clone();
                                                let cite = icon_action(
                                                    "quote",
                                                    "引用此回答",
                                                    move |_event: web_sys::MouseEvent| {
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
                                                    },
                                                );

                                                let identity_model = turn
                                                    .model
                                                    .clone()
                                                    .unwrap_or_else(|| model.get());
                                                view! {
                                                    <div class="turn">
                                                        <div class="question-row">
                                                            {refs}
                                                            <div class="question-stack">
                                                                <p class="turn-question">{turn.question.clone()}</p>
                                                            </div>
                                                        </div>
                                                        <div class="answer-block">
                                                        <div class="answer-head">
                                                            <div class="answer-identity">
                                                                {provider_tile(provider_preset.get(), 22)}
                                                                <span class="answer-model">{identity_model}</span>
                                                            </div>
                                                            <div class="question-actions">
                                                                {move || branch_nav()}
                                                                {edit}
                                                            </div>
                                                        </div>
                                                        {detail}
                                                        {body}
                                                        {answer_sources}
                                                        <div class="turn-actions">
                                                            {located}
                                                            {cite}
                                                            {icon_action(
                                                                "copy",
                                                                "复制",
                                                                move |_event: web_sys::MouseEvent| {
                                                                    copy_to_clipboard(&copied);
                                                                    notify.run("已复制".to_owned());
                                                                },
                                                            )}
                                                            {regenerate}
                                                            {continued}
                                                        </div>
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
                                            let label = if streaming_now && answer.get().trim().is_empty() {
                                                format!("思考中 · {:.1} 秒", think_seconds.get().max(0.1))
                                            } else {
                                                format!("思考 {:.1} 秒", think_seconds.get())
                                            };
                                            if streaming_now {
                                                view! {
                                                    <details
                                                        class="think-pill is-live"
                                                        open=move || reasoning_open.get()
                                                        on:toggle=move |event: web_sys::Event| {
                                                            if let Some(details) = event
                                                                .target()
                                                                .and_then(|target| target.dyn_into::<web_sys::HtmlDetailsElement>().ok())
                                                            {
                                                                reasoning_open.set(details.open());
                                                            }
                                                        }
                                                    >
                                                        <summary>
                                                            {icon("brain", 14)}
                                                            <span>{label}</span>
                                                            <span class="think-chevron">{icon("chevron-down", 12)}</span>
                                                        </summary>
                                                        <pre class="think-text">{reasoning.get()}</pre>
                                                    </details>
                                                }
                                                    .into_any()
                                            } else {
                                                view! {
                                                    <details class="think-pill">
                                                        <summary>
                                                            {icon("brain", 14)}
                                                            <span>{label}</span>
                                                            <span class="think-chevron">{icon("chevron-down", 12)}</span>
                                                        </summary>
                                                        <pre class="think-text">{reasoning.get()}</pre>
                                                    </details>
                                                }
                                                    .into_any()
                                            }
                                        };
                                        let refs = reference_badge(&current_context.get());
                                        view! {
                                            <div class="turn current" class:is-streaming=move || streaming.get()>
                                                <div class="question-row">
                                                    {refs}
                                                    <div class="question-stack">
                                                        <p class="turn-question">
                                                        {move || {
                                                            if continue_target.get().is_some() {
                                                                "继续生成…".to_owned()
                                                            } else {
                                                                current_question.get()
                                                            }
                                                        }}
                                                        </p>
                                                    </div>
                                                </div>
                                                <div class="answer-block">
                                                <div class="answer-identity">
                                                    {provider_tile(provider_preset.get(), 22)}
                                                    <span class="answer-model">
                                                        {move || {
                                                            session_model
                                                                .get()
                                                                .unwrap_or_else(|| model.get())
                                                        }}
                                                    </span>

                                                </div>
                                                {detail}
                                                <Show when=move || streaming.get() && answer.get().is_empty()>
                                                    <div class="answer-skeleton" aria-hidden="true">
                                                        <span></span>
                                                        <span></span>
                                                        <span></span>
                                                    </div>
                                                </Show>
                                                <div
                                                    class="answer-text markdown"
                                                    aria-live="polite"
                                                    inner_html=move || {
                                                        pdfscope_core::to_html(&answer.get())
                                                    }
                                                ></div>
                                                <Show when=move || !streaming.get()>
                                                    <div class="turn-actions">
                                                        {icon_action(
                                                            "copy",
                                                            "复制",
                                                            move |_event: web_sys::MouseEvent| {
                                                                copy_to_clipboard(&answer.get());
                                                                notify.run("已复制".to_owned());
                                                            },
                                                        )}
                                                    </div>
                                                </Show>
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

                        let total = quotes.len()
                            + shots.len()
                            + pictures.len()
                            + documents.len()
                            + citations.len()
                            + pages_selected.len()
                            + cited.len();

                        let quote_row = (!quotes.is_empty()).then(|| {
                            view! {
                                <div class="shelf-row">
                                    <span class="shelf-label">{icon("quote", 12)}<span>文字</span></span>
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
                                                            {icon("close", 12)}
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
                                    <span class="shelf-label">{icon("image", 12)}<span>截图</span></span>
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
                                                            {icon("close", 12)}
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
                                    <span class="shelf-label">{icon("image", 12)}<span>图片</span></span>
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
                                                            {icon("close", 12)}
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
                                    <span class="shelf-label">{icon("file", 12)}<span>文件</span></span>
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
                                                            {icon("close", 12)}
                                                        </span>
                                                    </div>
                                                }
                                            })
                                            .collect_view()}
                                    </div>
                                </div>
                            }
                        });

                        let ranges = page_ranges(&pages_selected);
                        let page_row = (!ranges.is_empty()).then(|| {
                            view! {
                                <div class="shelf-row">
                                    <span class="shelf-label">{icon("pages", 12)}<span>"页面"</span></span>
                                    <div class="shelf-track">
                                        {ranges
                                            .into_iter()
                                            .map(|(start, end)| {
                                                let label = if start == end {
                                                    format!("p.{start}")
                                                } else {
                                                    format!("p.{start}–{end}")
                                                };
                                                let drag_key = format!("page:{start}\np.{start}");
                                                view! {
                                                    <div
                                                        class="attach-chip"
                                                        draggable="true"
                                                        on:click=move |_| jump(start)
                                                        on:dragstart=move |event: web_sys::DragEvent| {
                                                            if let Some(data) = event.data_transfer() {
                                                                let _ = data.set_data("text/plain", &drag_key);
                                                                data.set_effect_allowed("copy");
                                                            }
                                                        }
                                                    >
                                                        <span class="chip-kind">{icon("page", 12)}</span>
                                                        <span class="chip-page">{label}</span>
                                                        <span
                                                            class="chip-x"
                                                            title="移除"
                                                            on:click=move |event: web_sys::MouseEvent| {
                                                                event.stop_propagation();
                                                                undo_push.run(());
                                                                for page in start..=end {
                                                                    let _ = composer(
                                                                        "removeToken",
                                                                        &[JsValue::from_str(&format!("page:{page}"))],
                                                                    );
                                                                }
                                                                page_set.update(|set| set.retain(|page| *page < start || *page > end));
                                                            }
                                                        >
                                                            {icon("close", 11)}
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
                                    <span class="shelf-label">{icon("link", 12)}<span>来源</span></span>
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
                                    <span class="shelf-label">{icon("sparkle", 12)}<span>回答</span></span>
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
                                                        <span class="chip-kind">{icon("sparkle", 12)}</span>
                                                        <span class="chip-text">{truncate(&answer.quote, 32)}</span>
                                                        <span
                                                            class="chip-x"
                                                            title="移除"
                                                            on:click=move |event: web_sys::MouseEvent| {
                                                                event.stop_propagation();
                                                                remove_answer(id);
                                                            }
                                                        >
                                                            {icon("close", 12)}
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
                                <div class="shelf-head">
                                    <span class="shelf-title">{format!("上下文 · {total} 项")}</span>
                                    <button
                                        class="shelf-clear"
                                        on:click=move |_| {
                                            undo_push.run(());
                                            passages.set(Vec::new());
                                            regions.set(Vec::new());
                                            images.set(Vec::new());
                                            files.set(Vec::new());
                                            sources.set(Vec::new());
                                            answers.set(Vec::new());
                                            page_set.update(|set| set.clear());
                                            // The shelf is the source of truth; the editor must not keep
                                            // pills whose attachment no longer exists.
                                            let _ = composer("clearTokens", &[]);
                                            notify.run("已清空上下文".to_owned());
                                        }
                                    >
                                        "清空"
                                    </button>
                                </div>
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
                                    <span class="mention-kind">{icon("sparkle", 12)}</span>
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
                    <div class="composer-card">
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
                    <div class="composer-toolbar">
                        <button
                            class="icon-button tool-at"
                            title="插入引用（输入 @）"
                            on:click=move |_| {
                                mention_open.set(true);
                                let _ = composer("focus", &[]);
                            }
                        >
                            {icon("at", 16)}
                        </button>
                        <button
                            class="icon-button attach-button tool-search"
                            class:active=move || web.get()
                            title="联网搜索（需要 Tavily Key）"
                            on:click=move |_| web.update(|on| *on = !*on)
                        >
                            <svg viewBox="0 0 24 24" width="17" height="17" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round">
                                <circle cx="12" cy="12" r="8.5" />
                                <path d="M3.5 12h17M12 3.5c2.4 2.3 3.6 5.1 3.6 8.5s-1.2 6.2-3.6 8.5c-2.4-2.3-3.6-5.1-3.6-8.5S9.6 5.8 12 3.5z" />
                            </svg>
                        </button>
                        <label class="icon-button attach-button tool-attach" title="附加图片、PDF、Office 或文本/代码文件">
    {paperclip()}
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
                        <div class="scope-icons toolbar-scope" role="group" aria-label="提问范围">
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
                                <img class="scope-art" src="/scope-icons/document.svg" alt="" draggable="false" />
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
                                <img class="scope-art" src="/scope-icons/page.svg" alt="" draggable="false" />
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
                                <img class="scope-art" src="/scope-icons/references.svg" alt="" draggable="false" />
                                <span class="scope-badge">
                                    {move || {
                                        let count = reference_count();
                                        if count == 0 { String::new() } else { count.to_string() }
                                    }}
                                </span>
                            </button>
                        </div>
                        <button
                            class="icon-button"
                            title="管理页面引用"
                            on:click=move |_| manager_open.set(true)
                        >
                            {icon("book", 17)}
                        </button>
                        <span class="toolbar-grow"></span>
                        <button
                            class="send-button"
                            class:is-stop=move || streaming.get()
                            title=move || {
                                if streaming.get() { "停止" } else if failed.get() { "重试" } else { "发送" }
                            }
                            disabled=move || !streaming.get() && question.get().trim().is_empty()
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
                                    icon("stop", 17)
                                } else if failed.get() {
                                    icon("retry", 17)
                                } else {
                                    icon("send", 17)
                                }
                            }}
                        </button>
                    </div>
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
                    provider_preset=provider_preset
                />

            </aside>
        }
}
