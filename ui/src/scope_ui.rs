//! The page-manager dialog: its thumbnails, drag range and clear action.

use leptos::prelude::*;

/// The manager card under a viewport coordinate. Pointer capture sends move/up events to the grid,
/// so `event.target()` is no longer a card; hit-test the point instead.
fn page_at(x: f64, y: f64) -> Option<u32> {
    web_sys::window()?
        .document()?
        .element_from_point(x as f32, y as f32)?
        .closest("[data-page]")
        .ok()
        .flatten()?
        .get_attribute("data-page")?
        .parse::<u32>()
        .ok()
}

#[component]
pub(crate) fn PageManager(
    open: RwSignal<bool>,
    pages: RwSignal<u32>,
    page_set: RwSignal<Vec<u32>>,
    scope_mode: RwSignal<u8>,
    undo_push: Callback<()>,
    page: RwSignal<u32>,
) -> impl IntoView {
    // start page, pointer x/y at press. Hover tracks the live range; subtract mirrors Alt.
    let manager_drag = RwSignal::new(Option::<(u32, f64, f64)>::None);
    let manager_hover = RwSignal::new(Option::<u32>::None);
    let manager_subtract = RwSignal::new(false);
    let grid_ref = NodeRef::<leptos::html::Div>::new();

    let toggle_page = Callback::new(move |(number, subtract): (u32, bool)| {
        let mut next = page_set.get_untracked();
        let before = next.clone();
        if subtract {
            pdfscope_core::remove_page(&mut next, number);
        } else {
            pdfscope_core::toggle_page(&mut next, number);
        }
        if next != before {
            undo_push.run(());
            page_set.set(next);
            scope_mode.set(2);
        }
    });

    let change_range = Callback::new(move |(start, end, subtract): (u32, u32, bool)| {
        let mut next = page_set.get_untracked();
        let before = next.clone();
        if subtract {
            pdfscope_core::subtract_range(&mut next, start, end);
        } else {
            pdfscope_core::add_range(&mut next, start, end);
        }
        if next != before {
            undo_push.run(());
            page_set.set(next);
            scope_mode.set(2);
        }
    });

    Effect::new(move |_| {
        if !open.get() {
            manager_drag.set(None);
            manager_hover.set(None);
            manager_subtract.set(false);
        }
    });

    let clear_drag = move || {
        manager_drag.set(None);
        manager_hover.set(None);
        manager_subtract.set(false);
    };

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
                        <div
                            class="manager-grid"
                            id="manager-grid"
                            node_ref=grid_ref
                            on:dragstart=move |event: web_sys::DragEvent| event.prevent_default()
                            on:pointermove=move |event: web_sys::PointerEvent| {
                                if manager_drag.get_untracked().is_none() {
                                    return;
                                }
                                manager_subtract.set(event.alt_key());
                                if let Some(target) =
                                    page_at(event.client_x() as f64, event.client_y() as f64)
                                {
                                    manager_hover.set(Some(target));
                                }
                            }
                            on:pointerup=move |event: web_sys::PointerEvent| {
                                let Some((start, start_x, start_y)) =
                                    manager_drag.get_untracked()
                                else {
                                    return;
                                };
                                if let Some(grid) = grid_ref.get() {
                                    let _ = grid.release_pointer_capture(event.pointer_id());
                                }
                                let target = page_at(
                                    event.client_x() as f64,
                                    event.client_y() as f64,
                                )
                                .or_else(|| manager_hover.get_untracked())
                                .unwrap_or(start);
                                let subtract =
                                    event.alt_key() || manager_subtract.get_untracked();
                                let moved = (event.client_x() as f64 - start_x).abs()
                                    + (event.client_y() as f64 - start_y).abs();
                                clear_drag();
                                if moved < 4.0 || target == start {
                                    toggle_page.run((start, subtract));
                                } else {
                                    change_range.run((start, target, subtract));
                                }
                            }
                            on:pointercancel=move |event: web_sys::PointerEvent| {
                                if let Some(grid) = grid_ref.get() {
                                    let _ = grid.release_pointer_capture(event.pointer_id());
                                }
                                clear_drag();
                            }

                        >
                            {(1..=pages.get().max(1))
                                .map(|number| {
                                    view! {
                                        <button
                                            class="thumb manager-thumb"
                                            data-page=number
                                            draggable="false"
                                            class:selected=move || page_set.get().contains(&number)
                                            class:current=move || page.get() == number
                                            class:drag-preview=move || {
                                                let Some((start, _, _)) = manager_drag.get() else {
                                                    return false;
                                                };
                                                let end = manager_hover.get().unwrap_or(start);
                                                let (low, high) =
                                                    if start <= end { (start, end) } else { (end, start) };
                                                (low..=high).contains(&number)
                                            }
                                            class:drag-subtract=move || manager_subtract.get()
                                            aria-pressed=move || page_set.get().contains(&number).to_string()
                                            on:dragstart=move |event: web_sys::DragEvent| event.prevent_default()
                                            on:pointerdown=move |event: web_sys::PointerEvent| {
                                                if event.button() != 0 {
                                                    return;
                                                }
                                                // The thumbnails are images inside buttons; without this the
                                                // browser starts a native image drag and the app's file-drop
                                                // layer shows its upload overlay instead of selecting pages.
                                                event.prevent_default();
                                                manager_drag.set(Some((
                                                    number,
                                                    event.client_x() as f64,
                                                    event.client_y() as f64,
                                                )));
                                                manager_hover.set(Some(number));
                                                manager_subtract.set(event.alt_key());
                                                if let Some(grid) = grid_ref.get() {
                                                    let _ =
                                                        grid.set_pointer_capture(event.pointer_id());
                                                }
                                            }
                                            on:keydown=move |event: web_sys::KeyboardEvent| {
                                                if event.key() == "Enter"
                                                    || event.key() == " "
                                                    || event.key() == "Spacebar"
                                                {
                                                    event.prevent_default();
                                                    toggle_page.run((number, false));
                                                }
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
