//! The page-manager dialog: its thumbnails, drag range and clear action.

use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[component]
pub(crate) fn PageManager(
    open: RwSignal<bool>,
    pages: RwSignal<u32>,
    page_set: RwSignal<Vec<u32>>,
    scope_mode: RwSignal<u8>,
    undo_push: Callback<()>,
    page: RwSignal<u32>,
) -> impl IntoView {
    // page, pointer x/y at press: a drag selects a range, a click toggles one page
    let manager_drag = RwSignal::new(Option::<(u32, f64, f64)>::None);

    view! {
            <Show when=move || open.get()>
                <div class="settings-overlay" on:click=move |event: web_sys::MouseEvent| {
                    if event.target() == event.current_target() {
                        open.set(false);
                    }
                }>
                    <div class="settings-dialog page-manager">
                        <header class="settings-head page-manager-head">
                            <div>
                                <h2>"页面引用"</h2>
                                <p class="page-manager-hint">
                                    "点击切换单页 · 拖拽选择范围 · Alt 拖拽减选"
                                </p>
                            </div>
                            <button class="icon-button" title="关闭" on:click=move |_| open.set(false)>
                                "✕"
                            </button>
                        </header>
                        <div class="manager-grid" id="manager-grid">
                            {(1..=pages.get().max(1))
                                .map(|number| {
                                    view! {
                                        <button
                                            class="thumb manager-thumb"
                                            data-page=number
                                            class:selected=move || page_set.get().contains(&number)
                                            class:current=move || page.get() == number
                                            aria-pressed=move || page_set.get().contains(&number).to_string()
                                            on:pointerdown=move |event: web_sys::PointerEvent| {
                                                manager_drag.set(Some((
                                                    number,
                                                    event.client_x() as f64,
                                                    event.client_y() as f64,
                                                )));
                                            }
                                            on:pointerup=move |event: web_sys::PointerEvent| {
                                                let Some((start, start_x, start_y)) = manager_drag.get() else {
                                                    return;
                                                };
                                                manager_drag.set(None);
                                                let subtract = event.alt_key();
                                                let moved = (event.client_x() as f64 - start_x).abs()
                                                    + (event.client_y() as f64 - start_y).abs();
                                                let target = event
                                                    .target()
                                                    .and_then(|target| target.dyn_into::<web_sys::Element>().ok())
                                                    .and_then(|element| element.closest("[data-page]").ok().flatten())
                                                    .and_then(|element| element.get_attribute("data-page"))
                                                    .and_then(|value| value.parse::<u32>().ok())
                                                    .unwrap_or(number);

                                                undo_push.run(());
                                                if moved < 4.0 || target == start {
                                                    page_set.update(|set| {
                                                        if subtract {
                                                            pdfscope_core::remove_page(set, start);
                                                        } else {
                                                            pdfscope_core::toggle_page(set, start);
                                                        }
                                                    });
                                                } else {
                                                    page_set.update(|set| {
                                                        if subtract {
                                                            pdfscope_core::subtract_range(set, start, target);
                                                        } else {
                                                            pdfscope_core::add_range(set, start, target);
                                                        }
                                                    });
                                                }
                                                scope_mode.set(2);
                                            }
                                        >
                                            <span class="thumb-number">{number}</span>
                                        </button>
                                    }
                                })
                                .collect_view()}
                        </div>
                        <footer class="settings-foot page-manager-foot">
                            <button
                                class="button"
                                prop:disabled=move || page_set.get().is_empty()
                                on:click=move |_| {
                                    undo_push.run(());
                                    page_set.set(Vec::new());
                                    scope_mode.set(1);
                                }
                            >
                                "清空页面引用"
                            </button>
                            <span class="settings-path">{move || format!("已选 {} 页", page_set.get().len())}</span>
                            <button class="button button-primary" on:click=move |_| open.set(false)>
                                "完成"
                            </button>
                        </footer>
                    </div>
                </div>
            </Show>

    }
}
