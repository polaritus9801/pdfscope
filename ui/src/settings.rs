//! The settings sheet: the connection, the model, the reading preferences.

use leptos::prelude::*;
use leptos::task::spawn_local;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::panel::{
    PRESET_LABELS, apply_font_size, apply_theme, bridge, flag_of, number_of, preset, text_of,
};

#[component]
pub(crate) fn SettingsSheet(
    open: RwSignal<bool>,
    configured: RwSignal<bool>,
    notify: Callback<String>,
    model: RwSignal<String>,
) -> impl IntoView {
    let testing = RwSignal::new(false);

    let test_result = RwSignal::new(Option::<String>::None);

    let form_preset = RwSignal::new("MiMo".to_owned());

    let form_protocol = RwSignal::new("openai".to_owned());

    let form_base = RwSignal::new("https://api.xiaomimimo.com/v1".to_owned());

    let form_model = RwSignal::new("mimo-v2.6-flash".to_owned());

    let form_key = RwSignal::new(String::new());

    let form_prompt = RwSignal::new(String::new());

    let form_theme = RwSignal::new("system".to_owned());

    let form_max = RwSignal::new(String::new());

    let form_max_error = RwSignal::new(Option::<String>::None);

    let form_font_size = RwSignal::new("14".to_owned());

    let form_remember = RwSignal::new(true);

    let form_zoom = RwSignal::new("100".to_owned());

    let form_mode = RwSignal::new("continuous".to_owned());

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
                if let Some(stored) = text_of(&value, "preset") {
                    form_preset.set(stored);
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
                        apply_theme(&form_theme.get());
                        apply_font_size(&form_font_size.get());
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
                                            <select
                                                on:change=move |event| {
                                                    let chosen = event_target_value(&event);
                                                    if let Some((protocol, base)) = preset(&chosen) {
                                                        form_protocol.set(protocol.to_owned());
                                                        form_base.set(base.to_owned());
                                                    }
                                                    form_preset.set(chosen);
                                                }
                                            >
                                                {PRESET_LABELS
                                                    .iter()
                                                    .map(|label| {
                                                        let value = *label;
                                                        let selected = move || form_preset.get() == value;
                                                        view! {
                                                            <option value=value selected=selected>
                                                                {value}
                                                            </option>
                                                        }
                                                    })
                                                    .collect_view()}
                                            </select>
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
