//! The settings sheet: the connection, the model, the reading preferences.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::icons::{icon, provider_tile};
use crate::panel::{
    PRESET_LABELS, apply_font_size, apply_theme, bridge, flag_of, number_of, preset, text_of,
};

/// Publishes the timeout to the bridge, in milliseconds; zero disables the watchdog.
fn apply_request_timeout(seconds: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let seconds = seconds.parse::<u64>().unwrap_or(0);
    let _ = js_sys::Reflect::set(
        &window,
        &JsValue::from_str("PDFScopeRequestTimeout"),
        &JsValue::from_f64((seconds * 1000) as f64),
    );
}

/// Splits the comma-separated model list into unique, trimmed names.
fn parse_choices(text: &str) -> Vec<String> {
    let mut choices: Vec<String> = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() || choices.iter().any(|choice| choice == part) {
            continue;
        }
        choices.push(part.to_owned());
    }
    choices
}

#[component]
pub(crate) fn SettingsSheet(
    open: RwSignal<bool>,
    configured: RwSignal<bool>,
    notify: Callback<String>,
    model: RwSignal<String>,
    page_limit: RwSignal<u32>,
    context_limit: RwSignal<u32>,
    model_choices: RwSignal<Vec<String>>,
    provider_preset: RwSignal<String>,
) -> impl IntoView {
    let testing = RwSignal::new(false);

    let test_result = RwSignal::new(Option::<String>::None);

    let form_preset = RwSignal::new("MiMo".to_owned());

    let form_protocol = RwSignal::new("openai".to_owned());

    let form_base = RwSignal::new("https://api.xiaomimimo.com/v1".to_owned());

    let form_model = RwSignal::new("mimo-v2.6-flash".to_owned());
    let form_model_choices = RwSignal::new(String::new());
    let preset_menu = RwSignal::new(false);

    let form_key = RwSignal::new(String::new());

    let form_prompt = RwSignal::new(String::new());

    let form_theme = RwSignal::new("system".to_owned());

    let form_max = RwSignal::new(String::new());

    let form_max_error = RwSignal::new(Option::<String>::None);

    let form_font_size = RwSignal::new("14".to_owned());

    let form_remember = RwSignal::new(true);

    let form_zoom = RwSignal::new("100".to_owned());

    let form_mode = RwSignal::new("continuous".to_owned());
    let form_page_limit = RwSignal::new(0_u32);
    let form_timeout = RwSignal::new("0".to_owned());
    let form_context_limit = RwSignal::new("128000".to_owned());
    let form_search_key = RwSignal::new(String::new());
    let form_search_provider = RwSignal::new("deepseek".to_owned());
    let search_key_stored = RwSignal::new(false);

    let form_spread_offset = RwSignal::new(false);

    let tab = RwSignal::new(0_u8);

    let form_error = RwSignal::new(Option::<String>::None);

    // Load the stored configuration once.
    Effect::new(move |_| {
        if let Some(promise) = bridge("loadSettings", &[]) {
            spawn_local(async move {
                let Ok(value) = JsFuture::from(js_sys::Promise::from(promise)).await else {
                    return;
                };
                configured.set(flag_of(&value, "has_key").unwrap_or(false));
                if let Some(stored) = text_of(&value, "model").filter(|text| !text.is_empty()) {
                    model.set(stored.clone());
                    form_model.set(stored);
                }
                let choices = text_of(&value, "model_choices").unwrap_or_default();
                model_choices.set(parse_choices(&choices));
                form_model_choices.set(choices);
                if let Some(stored) = text_of(&value, "preset") {
                    form_preset.set(stored.clone());
                    provider_preset.set(stored);
                }
                if let Some(stored) = text_of(&value, "protocol") {
                    form_protocol.set(stored);
                }
                if let Some(stored) = text_of(&value, "base_url") {
                    form_base.set(stored);
                }
                if let Some(stored) = text_of(&value, "system_prompt") {
                    form_prompt.set(stored);
                }
                if let Some(stored) = text_of(&value, "theme") {
                    apply_theme(&stored);
                    form_theme.set(stored);
                }
                form_font_size.set(
                    number_of(&value, "font_size")
                        .map(|size| (size as u16).to_string())
                        .unwrap_or_else(|| "14".to_owned()),
                );
                form_remember.set(flag_of(&value, "remember_position").unwrap_or(true));
                form_zoom.set(
                    number_of(&value, "default_zoom")
                        .map(|zoom| (zoom as u16).to_string())
                        .unwrap_or_else(|| "100".to_owned()),
                );
                form_mode
                    .set(text_of(&value, "page_mode").unwrap_or_else(|| "continuous".to_owned()));
                form_page_limit.set(
                    number_of(&value, "page_limit")
                        .map(|limit| limit as u32)
                        .unwrap_or(0),
                );
                page_limit.set(form_page_limit.get());
                form_timeout.set(
                    number_of(&value, "request_timeout")
                        .map(|seconds| (seconds as u32).to_string())
                        .unwrap_or_else(|| "0".to_owned()),
                );
                apply_request_timeout(&form_timeout.get());
                form_context_limit.set(
                    number_of(&value, "context_limit")
                        .map(|limit| (limit as u32).to_string())
                        .unwrap_or_else(|| "128000".to_owned()),
                );
                context_limit.set(form_context_limit.get().parse::<u32>().unwrap_or(128_000));
                search_key_stored.set(
                    js_sys::Reflect::get(&value, &JsValue::from_str("has_search_key"))
                        .ok()
                        .and_then(|flag| flag.as_bool())
                        .unwrap_or(false),
                );
                form_search_provider.set(
                    text_of(&value, "search_provider")
                        .filter(|provider| !provider.trim().is_empty())
                        .unwrap_or_else(|| "deepseek".to_owned()),
                );
                let _ = crate::call("setPageMode", &[JsValue::from_str(&form_mode.get())]);
                let _ = crate::call(
                    "setDefaultZoom",
                    &[JsValue::from_f64(
                        form_zoom.get().parse::<f64>().unwrap_or(100.0),
                    )],
                );
                apply_font_size(&form_font_size.get());
                let _ = crate::call(
                    "setRememberPosition",
                    &[JsValue::from_bool(form_remember.get())],
                );

                form_max.set(
                    number_of(&value, "max_tokens")
                        .map(|limit| (limit as u32).to_string())
                        .unwrap_or_default(),
                );
            });
        }
    });

    let build_payload = move || -> Option<js_sys::Object> {
        let payload = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("preset"),
            &JsValue::from_str(&form_preset.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("protocol"),
            &JsValue::from_str(&form_protocol.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("base_url"),
            &JsValue::from_str(&form_base.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("model"),
            &JsValue::from_str(&form_model.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("model_choices"),
            &JsValue::from_str(&form_model_choices.get()),
        );

        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("system_prompt"),
            &JsValue::from_str(&form_prompt.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("theme"),
            &JsValue::from_str(&form_theme.get()),
        );

        let limit = match form_max.get().trim() {
            "" => JsValue::NULL,
            text => match text.parse::<u32>() {
                Ok(value) if value > 0 => JsValue::from_f64(f64::from(value)),
                _ => {
                    form_max_error.set(Some("留空，或填一个正整数".to_owned()));
                    return None;
                }
            },
        };
        let _ = js_sys::Reflect::set(&payload, &JsValue::from_str("max_tokens"), &limit);
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("font_size"),
            &JsValue::from_f64(f64::from(form_font_size.get().parse::<u16>().unwrap_or(14))),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("remember_position"),
            &JsValue::from_bool(form_remember.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("spread_offset"),
            &JsValue::from_bool(form_spread_offset.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("page_mode"),
            &JsValue::from_str(&form_mode.get()),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("page_limit"),
            &JsValue::from_f64(f64::from(form_page_limit.get())),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("request_timeout"),
            &JsValue::from_f64(form_timeout.get().parse::<f64>().unwrap_or(0.0)),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("context_limit"),
            &JsValue::from_f64(f64::from(
                form_context_limit.get().parse::<u32>().unwrap_or(128_000),
            )),
        );
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("search_provider"),
            &JsValue::from_str(&form_search_provider.get()),
        );
        let search_key = form_search_key.get();
        if !search_key.trim().is_empty() {
            let _ = js_sys::Reflect::set(
                &payload,
                &JsValue::from_str("search_api_key"),
                &JsValue::from_str(&search_key),
            );
        }
        let _ = js_sys::Reflect::set(&payload, &JsValue::from_str("_placeholder"), &JsValue::NULL);
        let _ = js_sys::Reflect::set(
            &payload,
            &JsValue::from_str("default_zoom"),
            &JsValue::from_f64(form_zoom.get().parse::<f64>().unwrap_or(100.0)),
        );

        let key = form_key.get();
        if !key.trim().is_empty() {
            let _ = js_sys::Reflect::set(
                &payload,
                &JsValue::from_str("api_key"),
                &JsValue::from_str(&key),
            );
        }

        Some(payload)
    };

    let save = move |_| {
        let Some(payload) = build_payload() else {
            return;
        };

        if let Some(promise) = bridge("saveSettings", &[payload.into()]) {
            spawn_local(async move {
                match JsFuture::from(js_sys::Promise::from(promise)).await {
                    Ok(_) => {
                        form_error.set(None);
                        open.set(false);
                        configured.set(true);
                        model.set(form_model.get());
                        model_choices.set(parse_choices(&form_model_choices.get()));
                        apply_theme(&form_theme.get());
                        apply_font_size(&form_font_size.get());
                        page_limit.set(form_page_limit.get());
                        apply_request_timeout(&form_timeout.get());
                        context_limit
                            .set(form_context_limit.get().parse::<u32>().unwrap_or(128_000));
                        if !form_search_key.get().trim().is_empty() {
                            search_key_stored.set(true);
                        }
                        form_search_key.set(String::new());
                        let _ = crate::call(
                            "setRememberPosition",
                            &[JsValue::from_bool(form_remember.get())],
                        );
                        let _ = crate::call("setPageMode", &[JsValue::from_str(&form_mode.get())]);
                        let _ = crate::call(
                            "setDefaultZoom",
                            &[JsValue::from_f64(
                                form_zoom.get().parse::<f64>().unwrap_or(100.0),
                            )],
                        );
                        notify.run("设置已保存".to_owned());
                        form_key.set(String::new());
                    }
                    Err(error) => {
                        form_error.set(Some(
                            error.as_string().unwrap_or_else(|| "保存失败".to_owned()),
                        ));
                    }
                }
            });
        }
    };

    let test = move |_| {
        if testing.get() {
            return;
        }
        testing.set(true);
        test_result.set(None);
        let payload = build_payload();

        if let Some(promise) = bridge("testSettings", &[payload.into()]) {
            spawn_local(async move {
                match JsFuture::from(js_sys::Promise::from(promise)).await {
                    Ok(value) => {
                        test_result.set(Some(
                            value.as_string().unwrap_or_else(|| "连接正常".to_owned()),
                        ));
                    }
                    Err(error) => {
                        test_result.set(Some(
                            error.as_string().unwrap_or_else(|| "连接失败".to_owned()),
                        ));
                    }
                }
                testing.set(false);
            });
        } else {
            testing.set(false);
        }
    };

    view! {
            <Show when=move || open.get()>
                <div
                    class="settings-overlay"
                    on:click=move |event: web_sys::MouseEvent| {
                        // A click on the backdrop closes; a click inside the dialog does not.
                        if event.target() == event.current_target() {
                            open.set(false);
                        }
                    }
                >
                    <div class="settings-dialog">
                        <header class="settings-head">
                            <h2>"设置"</h2>
                            <button
                                class="icon-button"
                                title="关闭"
                                on:click=move |_| open.set(false)
                            >
                                <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round">
                                    <path d="M6 6l12 12M18 6L6 18" />
                                </svg>
                            </button>
                        </header>

                        <div class="settings-body">
                            <nav class="settings-nav">
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 0
                                    on:click=move |_| tab.set(0)
                                >
                                    "通用设置"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 1
                                    on:click=move |_| tab.set(1)
                                >
                                    "阅读"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 2
                                    on:click=move |_| tab.set(2)
                                >
                                    "模型"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 3
                                    on:click=move |_| tab.set(3)
                                >
                                    "提示词"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 4
                                    on:click=move |_| tab.set(4)
                                >
                                    "快捷键"
                                </button>
                                <button
                                    class="settings-tab"
                                    class:active=move || tab.get() == 5
                                    on:click=move |_| tab.set(5)
                                >
                                    "关于"
                                </button>
                            </nav>

                            <div class="settings-content">
                                <Show when=move || tab.get() == 0>
                                    <section class="settings-section">
                                        <h3>"外观"</h3>
                                        <p class="settings-hint">"选择界面的明暗，跟随系统会随 macOS 切换。"</p>
                                        <div class="theme-cards">
                                            {[
                                                ("system", "跟随系统", "<rect x=\"3\" y=\"4\" width=\"18\" height=\"13\" rx=\"2\"/><path d=\"M8 20h8M12 17v3\"/>"),
                                                ("light", "浅色", "<circle cx=\"12\" cy=\"12\" r=\"4\"/><path d=\"M12 2v2M12 20v2M2 12h2M20 12h2M5 5l1.5 1.5M17.5 17.5L19 19M19 5l-1.5 1.5M6.5 17.5L5 19\"/>"),
                                                ("dark", "深色", "<path d=\"M20 14a8 8 0 1 1-9.9-7.7A7 7 0 0 0 20 14Z\"/>"),
                                            ]
                                                .into_iter()
                                                .map(|(value, label, path)| {
                                                    view! {
                                                        <button
                                                            class="theme-card"
                                                            class:active=move || form_theme.get() == value
                                                            on:click=move |_| form_theme.set(value.to_owned())
                                                        >
                                                            <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" inner_html=path></svg>
                                                            <span>{label}</span>
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    </section>


                                </Show>

                                <Show when=move || tab.get() == 1>
                                    <section class="settings-section">
                                        <h3>"阅读"</h3>
                                        <p class="settings-hint">"重新打开文件时回到上次读到的位置。"</p>
                                        <label class="settings-row">
                                            <span>"翻页方式"</span>
                                            <select on:change=move |event| form_mode.set(event_target_value(&event))>
                                                <option value="continuous" selected=move || form_mode.get() == "continuous">
                                                    "连续滚动"
                                                </option>
                                                <option value="single" selected=move || form_mode.get() == "single">
                                                    "单页"
                                                </option>
                                                <option value="spread" selected=move || form_mode.get() == "spread">
                                                    "双页"
                                                </option>
                                            </select>
                                        </label>

                                        <label class="settings-row">
                                            <span>"每次最多发送页数"</span>
                                            <select on:change=move |event| {
                                                form_page_limit
                                                    .set(event_target_value(&event).parse().unwrap_or(0))
                                            }>
                                                <option value="0" selected=move || form_page_limit.get() == 0>
                                                    "全部"
                                                </option>
                                                <option value="20" selected=move || form_page_limit.get() == 20>
                                                    "20 页"
                                                </option>
                                                <option value="50" selected=move || form_page_limit.get() == 50>
                                                    "50 页"
                                                </option>
                                                <option value="100" selected=move || form_page_limit.get() == 100>
                                                    "100 页"
                                                </option>
                                                <option value="200" selected=move || form_page_limit.get() == 200>
                                                    "200 页"
                                                </option>
                                            </select>
                                        </label>
                                        <p class="settings-hint">
                                            "整份文档提问时，超过上限只发送前面的页；设小可以省 token。"
                                        </p>

                                        <label class="settings-row">
                                            <span>"双页奇偶偏移"</span>
                                            <input
                                                type="checkbox"
                                                prop:checked=move || form_spread_offset.get()
                                                on:change=move |event| {
                                                    form_spread_offset.set(event_target_checked(&event));
                                                }
                                            />
                                        </label>

                                        <label class="settings-row">
                                            <span>"默认缩放"</span>
                                            <select on:change=move |event| form_zoom.set(event_target_value(&event))>
                                                {[75_u16, 90, 100, 110, 125, 150]
                                                    .into_iter()
                                                    .map(|percent| {
                                                        let value = percent.to_string();
                                                        let selected = {
                                                            let value = value.clone();
                                                            move || form_zoom.get() == value
                                                        };
                                                        view! {
                                                            <option value=value.clone() selected=selected>
                                                                {format!("{percent}%")}
                                                            </option>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </select>
                                        </label>

                                        <div class="settings-row">
                                            <span>"恢复默认"</span>
                                            <button
                                                class="button"
                                                on:click=move |_| {
                                                    form_zoom.set("100".to_owned());
                                                    form_remember.set(true);
                                                }
                                            >
                                                "恢复阅读默认"
                                            </button>
                                        </div>

                                        <label class="settings-row">
                                            <span>"记住阅读位置"</span>
                                            <input
                                                type="checkbox"
                                                prop:checked=move || form_remember.get()
                                                on:change=move |event| {
                                                    form_remember.set(event_target_checked(&event));
                                                }
                                            />
                                        </label>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 4>
                                    <section class="settings-section">
                                        <h3>"快捷键"</h3>
                                        <dl class="shortcuts">
                                            <dt>"⌘O"</dt><dd>"打开 PDF"</dd>
                                            <dt>"⌘F"</dt><dd>"文档内查找"</dd>
                                            <dt>"⌘↑ / ⌘↓"</dt><dd>"跳到首页 / 末页"</dd>
                                            <dt>"[ / ]"</dt><dd>"上一页 / 下一页"</dd>
                                            <dt>"Enter"</dt><dd>"发送问题"</dd>
                                            <dt>"Shift / ⌘ + Enter"</dt><dd>"在问题里换行"</dd>
                                            <dt>"Esc"</dt><dd>"关闭查找或停止回答"</dd>
                                            <dt>"⌘⇧C"</dt><dd>"复制最后一条回答"</dd>
                                            <dt>"⌘⇧R"</dt><dd>"重新生成"</dd>
                                            <dt>"⌘⇧G"</dt><dd>"继续生成（断流后）"</dd>
                                        </dl>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 5>
                                    <section class="settings-section">
                                        <h3>"关于"</h3>
                                        <p class="settings-hint">"PDFScope 0.1.0 · 本地优先的 PDF 阅读与提问工具。"</p>
                                        <p class="settings-hint">
                                            "配置与密钥保存在本机应用目录，不会上传；只有提问时才会把引用内容发送给你选择的模型服务。"
                                        </p>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 2>
                                    <section class="settings-section">
                                        <h3>"服务"</h3>
                                        <p class="settings-hint">"任何 OpenAI 兼容或 Anthropic 兼容的接口都可以。"</p>

                                        <label class="settings-row">
                                            <span>"厂商"</span>
                                            <div class="preset-field">
                                                <button
                                                    type="button"
                                                    class="preset-trigger"
                                                    on:click=move |_| preset_menu.update(|open| *open = !*open)
                                                >
                                                    {move || provider_tile(form_preset.get(), 20)}
                                                    <span class="preset-trigger-label">
                                                        {move || form_preset.get()}
                                                    </span>
                                                    {icon("chevron-down", 14)}
                                                </button>
                                                <Show when=move || preset_menu.get()>
                                                    <div
                                                        class="preset-backdrop"
                                                        on:pointerdown=move |_| preset_menu.set(false)
                                                    ></div>
                                                    <div class="preset-menu">
                                                        {PRESET_LABELS
                                                            .iter()
                                                            .map(|label| {
                                                                let value = (*label).to_owned();
                                                                let chosen = value.clone();
                                                                let chosen_for_class = chosen.clone();
                                                                let selected_class = move || form_preset.get() == chosen_for_class;
                                                                let selected_show = move || form_preset.get() == chosen;
                                                                view! {
                                                                    <button
                                                                        type="button"
                                                                        class="preset-option"
                                                                        class:selected=selected_class
                                                                        on:click=move |_| {
                                                                            if let Some((protocol, base)) = preset(&value) {
                                                                                form_protocol.set(protocol.to_owned());
                                                                                form_base.set(base.to_owned());
                                                                            }
                                                                            form_preset.set(value.clone());
                                                                            preset_menu.set(false);
                                                                        }
                                                                    >
                                                                        {provider_tile((*label).to_owned(), 20)}
                                                                        <span class="preset-option-label">{*label}</span>
                                                                        <Show when=selected_show>
                                                                            {icon("check", 14)}
                                                                        </Show>
                                                                    </button>
                                                                }
                                                            })
                                                            .collect_view()}
                                                    </div>
                                                </Show>
                                            </div>
                                        </label>

                                        <label class="settings-row">
                                            <span>"接口地址"</span>
                                            <input
                                                type="text"
                                                prop:value=move || form_base.get()
                                                on:input=move |event| form_base.set(event_target_value(&event))
                                            />
                                        </label>

                                        <label class="settings-row">
                                            <span>"模型"</span>
                                            <input
                                                type="text"
                                                prop:value=move || form_model.get()
                                                on:input=move |event| form_model.set(event_target_value(&event))
                                            />
                                        </label>

                                        <label class="settings-row">
                                            <span>"可选模型"</span>
                                            <input
                                                type="text"
                                                placeholder="逗号分隔，例如 deepseek-flash, deepseek-v4-pro"
                                                prop:value=move || form_model_choices.get()
                                                on:input=move |event| {
                                                    form_model_choices.set(event_target_value(&event))
                                                }
                                            />
                                        </label>
                                        <p class="settings-hint">
                                            "填写后每个对话可以在顶栏切换模型；留空则固定使用上面的模型。"
                                        </p>
                                        <div class="model-choice-list">
                                            {move || {
                                                parse_choices(&form_model_choices.get())
                                                    .into_iter()
                                                    .map(|name| {
                                                        view! {
                                                            <span class="model-choice">
                                                                {provider_tile(form_preset.get(), 16)}
                                                                <span>{name}</span>
                                                            </span>
                                                        }
                                                    })
                                                    .collect_view()
                                            }}
                                        </div>

                                        <label class="settings-row">
                                            <span>"响应超时（秒）"</span>
                                            <input
                                                type="number"
                                                min="0"
                                                prop:value=move || form_timeout.get()
                                                on:input=move |event| form_timeout.set(event_target_value(&event))
                                            />
                                        </label>
                                        <p class="settings-hint">
                                            "0 表示不设限：模型思考多久都等。只有网络卡死时才会用到。"
                                        </p>

                                        <label class="settings-row">
                                            <span>"上下文上限（tokens）"</span>
                                            <input
                                                type="number"
                                                min="0"
                                                prop:value=move || form_context_limit.get()
                                                on:input=move |event| {
                                                    form_context_limit.set(event_target_value(&event))
                                                }
                                            />
                                        </label>
                                        <p class="settings-hint">
                                            "发送前按估算的上下文拦截；0 表示不检查。"
                                        </p>

                                        <div class="provider-grid" role="radiogroup" aria-label="联网搜索服务">
                                            {[
                                                ("deepseek", "DeepSeek Search", "复用模型 Key"),
                                                ("tavily", "Tavily", "独立 Key"),
                                                ("brave", "Brave", "独立 Key"),
                                                ("exa", "Exa", "独立 Key"),
                                                ("serper", "Serper", "Google 结果"),
                                            ]
                                                .into_iter()
                                                .map(|(id, name, hint)| {
                                                    let id_value = id.to_owned();
                                                    let selected = move || form_search_provider.get() == id;
                                                    view! {
                                                        <button
                                                            class="provider-option"
                                                            class:selected=selected
                                                            role="radio"
                                                            aria-checked=move || selected().to_string()
                                                            on:click=move |_| form_search_provider.set(id_value.clone())
                                                        >
                                                            {provider_tile(name.to_owned(), 26)}
                                                            <span class="provider-option-text">
                                                                <strong>{name}</strong>
                                                                <small>{hint}</small>
                                                            </span>
                                                            <Show when=selected>{icon("check", 14)}</Show>
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                        <label class="settings-row">
                                            <span>
                                                "搜索 API Key"
                                                <Show when=move || search_key_stored.get()>
                                                    <em class="settings-badge">"已配置"</em>
                                                </Show>
                                            </span>
                                            <input
                                                type="password"
                                                placeholder=move || {
                                                    if search_key_stored.get() {
                                                        "已保存 · 留空保持不变"
                                                    } else if form_search_provider.get() == "deepseek" {
                                                        "可留空 · 复用模型 API Key"
                                                    } else {
                                                        "填写该服务的 Key"
                                                    }
                                                }
                                                prop:value=move || form_search_key.get()
                                                on:input=move |event| {
                                                    form_search_key.set(event_target_value(&event))
                                                }
                                            />
                                        </label>
                                        <p class="settings-hint">
                                            "DeepSeek Search 复用模型 Key；其余服务各自填写。保存后生效。"
                                        </p>

                                    <section class="settings-section">
                                        <h3>"字号"</h3>
                                        <p class="settings-hint">"回答正文的大小。"</p>
                                        <label class="settings-row">
                                            <span>"字号"</span>
                                            <select on:change=move |event| form_font_size.set(event_target_value(&event))>
                                                {(12..=18)
                                                    .map(|size| {
                                                        let value = size.to_string();
                                                        let selected = {
                                                            let value = value.clone();
                                                            move || form_font_size.get() == value
                                                        };
                                                        view! {
                                                            <option value=value.clone() selected=selected>
                                                                {format!("{size} px")}
                                                            </option>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </select>
                                        </label>
                                    </section>

                                    <section class="settings-section">
                                        <h3>"API Key"</h3>
                                        <p class="settings-hint">"保存在本机配置目录的 api-key 文件，权限 0600，只有这个用户能读。"</p>
                                        <label class="settings-row">
                                            <span>"密钥"</span>
                                            <input
                                                type="password"
                                                placeholder="留空表示沿用已保存的 key"
                                                prop:value=move || form_key.get()
                                                on:input=move |event| form_key.set(event_target_value(&event))
                                            />
                                        </label>
                                    </section>

                                        <label class="settings-row">
                                            <span>"最大输出"</span>
                                            <input
                                                type="number"
                                                min="1"
                                                placeholder="留空 = 不限制"
                                                prop:value=move || form_max.get()
                                                on:input=move |event| {
                                                    form_max_error.set(None);
                                                    form_max.set(event_target_value(&event));
                                                }
                                            />
                                        </label>
                                        {move || {
                                            form_max_error
                                                .get()
                                                .map(|message| {
                                                    view! { <p class="field-error">{message}</p> }
                                                })
                                        }}

                                        <div class="settings-row">
                                            <span>"连接"</span>
                                            <button
                                                class="button"
                                                on:click=test
                                                prop:disabled=move || testing.get()
                                            >
                                                {move || if testing.get() { "测试中…" } else { "测试连接" }}
                                            </button>
                                        </div>
                                    </section>
                                </Show>

                                <Show when=move || tab.get() == 3>
                                    <section class="settings-section">
                                        <h3>"系统提示词"</h3>
                                        <p class="settings-hint">"每次提问都会带上这段指令。清空会恢复默认。"</p>
                                        <button
                                            class="button"
                                            on:click=move |_| form_prompt.set(String::new())
                                        >
                                            "恢复默认提示词"
                                        </button>
                                        <textarea
                                            class="prompt-field"
                                            prop:value=move || form_prompt.get()
                                            on:input=move |event| form_prompt.set(event_target_value(&event))
                                        ></textarea>
                                    </section>
                                </Show>

                                {move || {
                                    form_error
                                        .get()
                                        .map(|message| view! { <p class="error settings-error">{message}</p> })
                                }}
                                {move || {
                                    test_result
                                        .get()
                                        .map(|message| view! { <p class="settings-ok">{message}</p> })
                                }}
                            </div>
                        </div>

                        <footer class="settings-foot">
                            <span class="settings-path">"配置保存在本机，不会上传。"</span>
                            <button class="button" on:click=move |_| open.set(false)>
                                "取消"
                            </button>
                            <button class="button button-primary" on:click=save>
                                "保存"
                            </button>
                        </footer>
                    </div>
                </div>
            </Show>
    }
}
