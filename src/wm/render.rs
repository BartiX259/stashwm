use super::{ActionButton, StashArea, WMEngine, WMState, layout::*};
use crate::backend::render::{CustomRenderElements, OutputRenderElements};
use crate::backend::shell::{WindowElement, WindowRenderElement};
use crate::wm::rect::RenderedRect;
use crate::wm::svg::Icon;
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::{
    backend::renderer::{
        Color32F, ImportAll, ImportMem, Renderer,
        element::{
            Id, Kind,
            solid::SolidColorRenderElement,
            utils::{ConstrainAlign, ConstrainScaleBehavior},
        },
        utils::CommitCounter,
    },
    desktop::space::{ConstrainBehavior, ConstrainReference, constrain_space_element},
    output::Output,
    utils::{Logical, Point, Rectangle, Scale, Size},
};

impl WMEngine {
    pub fn render_overlays<R>(
        &mut self,
        renderer: &mut R,
        output: &Output,
        elements: &mut Vec<OutputRenderElements<R, WindowRenderElement<R>>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + Send + 'static,
    {
        let scale = Scale::from(output.current_scale().fractional_scale());

        match &self.state {
            WMState::Normal { active_window } => {
                if let Some(window) = active_window {
                    let rect = self
                        .anim
                        .get_animating_rect(window)
                        .or(self.get_window_geometry(window));
                    if let Some(r) = rect {
                        push_outline_rect(elements, r, scale, OUTLINE_WIDTH, [0.1, 0.9, 0.9, 1.0]);
                    }
                }
            }
            WMState::WindowSelected {
                window,
                hovered_window,
                hovered_button,
            } => {
                // Action Buttons
                if let Some(win_rect) = self.get_window_geometry(window) {
                    for (action, rect) in get_action_buttons(win_rect) {
                        let is_hov = *hovered_button == Some(action);

                        let icon = match action {
                            ActionButton::Close => Icon::Close,
                            ActionButton::Minimize => Icon::Minimize,
                            ActionButton::Maximize => Icon::Maximize,
                        };
                        let icon_rect = padded(rect, icon_padding(rect));
                        let icon_buf = self.svg.render(icon, icon_rect.size, [1.0, 1.0, 1.0, 1.0]);
                        push_memory_buffer(elements, renderer, &icon_buf, icon_rect.loc, scale);

                        let (fill_col, border_col) = match action {
                            ActionButton::Maximize => {
                                if is_hov {
                                    ([0.18, 0.58, 0.82, 0.95], [0.5, 0.85, 1.0, 0.95])
                                } else {
                                    ([0.1, 0.4, 0.6, 0.85], [1.0, 1.0, 1.0, 0.0])
                                }
                            }
                            ActionButton::Minimize => {
                                if is_hov {
                                    ([0.85, 0.65, 0.15, 0.95], [1.0, 0.85, 0.4, 0.95])
                                } else {
                                    ([0.65, 0.45, 0.1, 0.85], [1.0, 1.0, 1.0, 0.0])
                                }
                            }
                            ActionButton::Close => {
                                if is_hov {
                                    ([0.9, 0.18, 0.18, 0.95], [1.0, 0.5, 0.5, 0.95])
                                } else {
                                    ([0.7, 0.1, 0.1, 0.85], [1.0, 1.0, 1.0, 0.0])
                                }
                            }
                        };

                        let btn = self.rect.render(rect.size, 12.0, fill_col, 1.0, border_col);
                        push_rendered_rect(elements, renderer, &btn, rect.loc, scale);
                    }

                    // Semi-transparent overlay
                    push_solid_rect(elements, win_rect, scale, [0.05, 0.06, 0.08, 0.8]);
                }

                // Hover outline
                if let Some(target) = hovered_window {
                    if let Some(target_rect) = self.get_window_geometry(target) {
                        push_outline_rect(
                            elements,
                            target_rect,
                            scale,
                            OUTLINE_WIDTH,
                            [0.2, 0.8, 0.5, 0.9],
                        );
                    }
                }
            }
            WMState::StashOpened { .. } => {}
        }
        // Stash
        let mut layout = calculate_stash_layout(self.screen_size, self.stashed_windows.len());
        let stash_progress = self.anim.stash_progress();
        if stash_progress > 0.0 {
            let offscreen_dy =
                ((1.0 - stash_progress) * (layout.main_rect.size.h as f32 + 100.0)).round() as i32;
            layout.offset_y(offscreen_dy);
            let mouse_area = match &self.state {
                WMState::StashOpened { mouse_area } => mouse_area,
                _ => &StashArea::Outside,
            };
            // Close buttons
            for (i, win) in self.stashed_windows.iter().enumerate() {
                if let Some(preview) = layout.previews.get(i) {
                    let icon_rect =
                        padded(preview.close_button, icon_padding(preview.close_button));
                    let icon_buf =
                        self.svg
                            .render(Icon::Close, icon_rect.size, [1.0, 1.0, 1.0, 1.0]);
                    push_memory_buffer(elements, renderer, &icon_buf, icon_rect.loc, scale);

                    let is_close_hov = *mouse_area == StashArea::PreviewClose(win.clone());
                    if is_close_hov {
                        let close_bg = self.rect.render(
                            preview.close_button.size,
                            4.0,
                            [0.88, 0.16, 0.16, 0.95],
                            0.0,
                            [0.0, 0.0, 0.0, 0.0],
                        );
                        push_rendered_rect(
                            elements,
                            renderer,
                            &close_bg,
                            preview.close_button.loc,
                            scale,
                        );
                    }
                }
            }

            // Titles above previewAs
            for (i, win) in self.stashed_windows.iter().enumerate() {
                if let Some(preview) = layout.previews.get(i) {
                    let title = get_window_title(win);
                    let title_pos =
                        Point::new(preview.main_rect.loc.x + 10, preview.main_rect.loc.y + 6);
                    let text = self.text.render(&title, 12.0, [0.9, 0.9, 0.95, 1.0]);
                    push_memory_buffer(elements, renderer, &text.buffer, title_pos, scale);
                }
            }

            // Window previews
            let constrain_behavior = ConstrainBehavior {
                reference: ConstrainReference::Geometry,
                behavior: ConstrainScaleBehavior::Fit,
                align: ConstrainAlign::CENTER,
            };
            for (i, win) in self.stashed_windows.iter().enumerate() {
                if let Some(preview) = layout.previews.get(i) {
                    let preview_elements = constrain_space_element(
                        renderer,
                        win,
                        preview.preview_rect.loc,
                        1.0,
                        scale.x,
                        preview.preview_rect,
                        constrain_behavior,
                    );
                    elements.extend(preview_elements.map(OutputRenderElements::Preview));
                }
            }

            // Preview outlines and backgrounds
            for (i, win) in self.stashed_windows.iter().enumerate() {
                if let Some(preview) = layout.previews.get(i) {
                    let is_hov = *mouse_area == StashArea::Preview(win.clone())
                        || *mouse_area == StashArea::PreviewClose(win.clone());

                    let (fill_col, border_col) = if is_hov {
                        ([0.12, 0.15, 0.20, 0.95], [0.2, 0.7, 0.95, 0.95])
                    } else {
                        ([0.1, 0.12, 0.16, 0.9], [1.0, 1.0, 1.0, 0.12])
                    };

                    let card =
                        self.rect
                            .render(preview.main_rect.size, 10.0, fill_col, 1.0, border_col);
                    push_rendered_rect(elements, renderer, &card, preview.main_rect.loc, scale);
                }
            }

            // Restore/close all
            let restore_all_text = self.text.render("Restore All", 12.0, [1.0, 1.0, 1.0, 1.0]);
            push_memory_buffer(
                elements,
                renderer,
                &restore_all_text.buffer,
                restore_all_text.centered_in(layout.restore_all),
                scale,
            );

            let close_all_text = self.text.render("Close All", 12.0, [1.0, 1.0, 1.0, 1.0]);
            push_memory_buffer(
                elements,
                renderer,
                &close_all_text.buffer,
                close_all_text.centered_in(layout.close_all),
                scale,
            );

            let is_restore_hov = *mouse_area == StashArea::RestoreAll;
            let (res_fill, res_border) = if is_restore_hov {
                ([0.2, 0.75, 0.42, 0.95], [0.4, 0.95, 0.6, 0.95])
            } else {
                ([0.15, 0.5, 0.3, 0.85], [1.0, 1.0, 1.0, 0.18])
            };
            let restore_btn =
                self.rect
                    .render(layout.restore_all.size, 14.0, res_fill, 1.0, res_border);
            push_rendered_rect(
                elements,
                renderer,
                &restore_btn,
                layout.restore_all.loc,
                scale,
            );

            let is_close_all_hov = *mouse_area == StashArea::CloseAll;
            let (cls_fill, cls_border) = if is_close_all_hov {
                ([0.88, 0.22, 0.22, 0.95], [1.0, 0.45, 0.45, 0.95])
            } else {
                ([0.65, 0.18, 0.18, 0.85], [1.0, 1.0, 1.0, 0.18])
            };
            let close_btn =
                self.rect
                    .render(layout.close_all.size, 14.0, cls_fill, 1.0, cls_border);
            push_rendered_rect(elements, renderer, &close_btn, layout.close_all.loc, scale);

            // Background
            let stash_bg = self.rect.render(
                layout.main_rect.size,
                14.0,
                [0.06, 0.07, 0.1, 0.95],
                1.0,
                [1.0, 1.0, 1.0, 0.1],
            );
            push_rendered_rect(elements, renderer, &stash_bg, layout.main_rect.loc, scale);
        }

        // for rect in &self.visible_rects {
        //     push_outline_rect(elements, *rect, scale, OUTLINE_WIDTH, [0.4, 0.4, 0.4, 1.0]);
        // }

        // Outlines and animations
        let constrain_behavior = ConstrainBehavior {
            reference: ConstrainReference::Geometry,
            behavior: ConstrainScaleBehavior::Stretch,
            align: ConstrainAlign::CENTER,
        };
        for window in self.visible_windows.iter() {
            let animating_rect = self.anim.get_animating_rect(window);
            if let Some(rect) = animating_rect {
                let preview_elements = constrain_space_element(
                    renderer,
                    window,
                    rect.loc,
                    1.0,
                    scale.x,
                    rect,
                    constrain_behavior,
                );
                elements.extend(preview_elements.map(OutputRenderElements::Preview));
            }
            if let Some(rect) = animating_rect.or(self.get_window_geometry(window)) {
                let border_color = [0.4, 0.4, 0.4, 1.0];
                push_outline_rect(elements, rect, scale, OUTLINE_WIDTH, border_color);
            }
        }
        for (win, render_rect) in self.anim.get_exiting_rects() {
            let preview_elements = constrain_space_element(
                renderer,
                &win,
                render_rect.loc,
                1.0,
                scale.x,
                render_rect,
                constrain_behavior,
            );
            elements.extend(preview_elements.map(OutputRenderElements::Preview));
            push_outline_rect(
                elements,
                render_rect,
                scale,
                OUTLINE_WIDTH,
                [0.4, 0.4, 0.4, 0.8],
            );
        }
    }
}

fn push_rendered_rect<R>(
    elements: &mut Vec<OutputRenderElements<R, WindowRenderElement<R>>>,
    renderer: &mut R,
    rendered: &RenderedRect,
    inner_loc: Point<i32, Logical>,
    scale: Scale<f64>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    push_memory_buffer(
        elements,
        renderer,
        &rendered.buffer,
        rendered.location(inner_loc),
        scale,
    );
}

fn push_solid_rect<R>(
    elements: &mut Vec<OutputRenderElements<R, WindowRenderElement<R>>>,
    rect: Rectangle<i32, Logical>,
    scale: Scale<f64>,
    color: [f32; 4],
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + 'static,
{
    let physical_rect = rect.to_physical_precise_round(scale);
    let color32 = Color32F::new(color[0], color[1], color[2], color[3]);

    let solid = SolidColorRenderElement::new(
        Id::new(),
        physical_rect,
        CommitCounter::default(),
        color32,
        Kind::Unspecified,
    );

    elements.push(OutputRenderElements::Custom(CustomRenderElements::Solid(
        solid,
    )));
}

fn push_outline_rect<R>(
    elements: &mut Vec<OutputRenderElements<R, WindowRenderElement<R>>>,
    mut rect: Rectangle<i32, Logical>,
    scale: Scale<f64>,
    thickness: i32,
    color: [f32; 4],
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + 'static,
{
    rect.loc.x -= thickness;
    rect.loc.y -= thickness;
    rect.size.w += thickness * 2;
    rect.size.h += thickness * 2;
    let side_h = rect.size.h - thickness * 2;
    let side_y = rect.loc.y + thickness;
    let borders = [
        Rectangle::new(rect.loc, Size::new(rect.size.w, thickness)),
        Rectangle::new(
            Point::new(rect.loc.x, rect.loc.y + rect.size.h - thickness),
            Size::new(rect.size.w, thickness),
        ),
        Rectangle::new(Point::new(rect.loc.x, side_y), Size::new(thickness, side_h)),
        Rectangle::new(
            Point::new(rect.loc.x + rect.size.w - thickness, side_y),
            Size::new(thickness, side_h),
        ),
    ];
    for b in borders {
        push_solid_rect(elements, b, scale, color);
    }
}

fn push_memory_buffer<R>(
    elements: &mut Vec<OutputRenderElements<R, WindowRenderElement<R>>>,
    renderer: &mut R,
    buffer: &MemoryRenderBuffer,
    location: Point<i32, Logical>,
    scale: Scale<f64>,
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + Send + 'static,
{
    let elem = MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        location.to_f64().to_physical(scale),
        buffer,
        None,
        None,
        None,
        Kind::Unspecified,
    )
    .unwrap();

    elements.push(OutputRenderElements::Custom(CustomRenderElements::Memory(
        elem,
    )));
}

fn get_window_title(window: &WindowElement) -> String {
    if let Some(surface) = window.wl_surface() {
        let title = smithay::wayland::compositor::with_states(&surface, |states| {
            states
                .data_map
                .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().unwrap().title.clone())
        });
        if let Some(t) = title {
            if !t.is_empty() {
                return t;
            }
        }
    }
    "Window".to_string()
}
