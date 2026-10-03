use super::{ActionButton, StashArea, WMEngine, WMState, layout::*};
use crate::backend::render::{CustomRenderElements, OutputRenderElements};
use crate::backend::shell::{WindowElement, WindowRenderElement};
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
                    if let Some(win_rect) = self.get_window_geometry(window) {
                        push_outline_rect(
                            elements,
                            win_rect,
                            scale,
                            OUTLINE_WIDTH,
                            [0.1, 0.9, 0.9, 1.0],
                        );
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
                        let icon_buf = self.svg_renderer.render_svg(
                            icon,
                            icon_rect.size,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        push_memory_buffer(elements, renderer, &icon_buf, icon_rect.loc, scale);

                        let color = match action {
                            ActionButton::Maximize => {
                                if is_hov {
                                    [0.2, 0.7, 0.9, 1.0]
                                } else {
                                    [0.1, 0.4, 0.6, 0.9]
                                }
                            }
                            ActionButton::Minimize => {
                                if is_hov {
                                    [0.9, 0.7, 0.2, 1.0]
                                } else {
                                    [0.7, 0.5, 0.1, 0.9]
                                }
                            }
                            ActionButton::Close => {
                                if is_hov {
                                    [1.0, 0.2, 0.2, 1.0]
                                } else {
                                    [0.8, 0.1, 0.1, 0.9]
                                }
                            }
                        };
                        push_solid_rect(elements, rect, scale, color);
                    }
                    // if hovered_window.is_none() {
                    //     push_outline_rect(
                    //         elements,
                    //         win_rect,
                    //         scale,
                    //         OUTLINE_WIDTH,
                    //         [0.2, 0.8, 0.5, 0.9],
                    //     );
                    // }

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
            WMState::StashOpened { mouse_area } => {
                let layout = calculate_stash_layout(self.screen_size, self.stashed_windows.len());

                // Close buttons
                for (i, win) in self.stashed_windows.iter().enumerate() {
                    if let Some(preview) = layout.previews.get(i) {
                        let icon_rect =
                            padded(preview.close_button, icon_padding(preview.close_button));
                        let icon_buf = self.svg_renderer.render_svg(
                            Icon::Close,
                            icon_rect.size,
                            [1.0, 1.0, 1.0, 1.0],
                        );
                        push_memory_buffer(elements, renderer, &icon_buf, icon_rect.loc, scale);
                        let is_close_hov = *mouse_area == StashArea::PreviewClose(win.clone());
                        let col = if is_close_hov {
                            [1.0, 0.2, 0.2, 1.0]
                        } else {
                            [0.6, 0.2, 0.2, 0.8]
                        };
                        push_solid_rect(elements, preview.close_button, scale, col);
                    }
                }

                // Titles above previewAs
                for (i, win) in self.stashed_windows.iter().enumerate() {
                    if let Some(preview) = layout.previews.get(i) {
                        let title = get_window_title(win);
                        let title_pos =
                            Point::new(preview.main_rect.loc.x + 10, preview.main_rect.loc.y + 6);
                        let text_buffer =
                            self.text_renderer
                                .render_text(&title, 12.0, [0.9, 0.9, 0.95, 1.0]);
                        push_memory_buffer(elements, renderer, &text_buffer, title_pos, scale);
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
                        if *mouse_area == StashArea::Preview(win.clone())
                            || *mouse_area == StashArea::PreviewClose(win.clone())
                        {
                            push_outline_rect(
                                elements,
                                preview.main_rect,
                                scale,
                                2,
                                [0.2, 0.7, 0.9, 1.0],
                            );
                        }
                        push_solid_rect(elements, preview.main_rect, scale, [0.1, 0.12, 0.16, 0.9]);
                    }
                }

                // Restore/close all
                let restore_all_text =
                    self.text_renderer
                        .render_text("Restore All", 12.0, [1.0, 1.0, 1.0, 1.0]);
                push_memory_buffer(
                    elements,
                    renderer,
                    &restore_all_text,
                    Point::new(layout.restore_all.loc.x + 12, layout.restore_all.loc.y + 6),
                    scale,
                );

                let close_all_text =
                    self.text_renderer
                        .render_text("Close All", 12.0, [1.0, 1.0, 1.0, 1.0]);
                push_memory_buffer(
                    elements,
                    renderer,
                    &close_all_text,
                    Point::new(layout.close_all.loc.x + 16, layout.close_all.loc.y + 6),
                    scale,
                );

                let restore_color = if *mouse_area == StashArea::RestoreAll {
                    [0.2, 0.8, 0.4, 1.0]
                } else {
                    [0.15, 0.5, 0.3, 0.9]
                };
                push_solid_rect(elements, layout.restore_all, scale, restore_color);

                let close_color = if *mouse_area == StashArea::CloseAll {
                    [1.0, 0.3, 0.3, 1.0]
                } else {
                    [0.7, 0.2, 0.2, 0.9]
                };
                push_solid_rect(elements, layout.close_all, scale, close_color);

                // Background
                push_solid_rect(elements, layout.main_rect, scale, [0.06, 0.07, 0.1, 0.95]);
            }
        }
        for rect in master_slave_layout(self.screen_size, self.visible_windows.len()) {
            push_outline_rect(elements, rect, scale, OUTLINE_WIDTH, [0.4, 0.4, 0.4, 1.0]);
        }
    }
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
