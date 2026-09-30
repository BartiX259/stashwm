use super::{ActionButton, StashArea, WMState, WMEngine, layout::*};
use crate::backend::render::{CustomRenderElements, OutputRenderElements};
use crate::backend::shell::WindowRenderElement;
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
        &self,
        renderer: &mut R,
        output: &Output,
        elements: &mut Vec<OutputRenderElements<R, WindowRenderElement<R>>>,
    ) where
        R: Renderer + ImportAll + ImportMem,
        R::TextureId: Clone + 'static,
    {
        let scale = Scale::from(output.current_scale().fractional_scale());

        match &self.state {
            WMState::Normal { .. } => {}
            WMState::WindowSelected {
                window,
                hovered_window,
                hovered_button,
            } => {
                // Action Buttons
                if let Some(win_rect) = self.get_window_geometry(window) {
                    for (action, rect) in get_action_buttons(win_rect) {
                        let is_hov = *hovered_button == Some(action);
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

                    // Semi-transparent overlay
                    push_solid_rect(elements, win_rect, scale, [0.05, 0.06, 0.08, 0.8]);
                }

                // Hover outline
                if let Some(target) = hovered_window {
                    if let Some(target_rect) = self.get_window_geometry(target) {
                        push_outline_rect(elements, target_rect, scale, 3, [0.2, 0.8, 0.5, 0.9]);
                    }
                }
            }
            WMState::StashOpened { mouse_area } => {
                let layout = calculate_stash_layout(self.screen_size, self.stashed_windows.len());

                // Close buttons
                for (i, win) in self.stashed_windows.iter().enumerate() {
                    if let Some(preview) = layout.previews.get(i) {
                        let is_close_hov = *mouse_area == StashArea::PreviewClose(win.clone());
                        let col = if is_close_hov {
                            [1.0, 0.2, 0.2, 1.0]
                        } else {
                            [0.6, 0.2, 0.2, 0.8]
                        };
                        push_solid_rect(elements, preview.close_button, scale, col);
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
                            preview.main_rect.loc,
                            1.0,
                            scale.x,
                            preview.main_rect,
                            constrain_behavior,
                        );
                        elements.extend(preview_elements.map(OutputRenderElements::Preview));
                    }
                }

                // Preview outlines and backgrounds
                for (i, win) in self.stashed_windows.iter().enumerate() {
                    if let Some(preview) = layout.previews.get(i) {
                        if *mouse_area == StashArea::Preview(win.clone()) {
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
                let restore_col = if *mouse_area == StashArea::RestoreAll {
                    [0.2, 0.8, 0.4, 1.0]
                } else {
                    [0.15, 0.5, 0.3, 0.9]
                };
                push_solid_rect(elements, layout.restore_all, scale, restore_col);

                let close_col = if *mouse_area == StashArea::CloseAll {
                    [1.0, 0.3, 0.3, 1.0]
                } else {
                    [0.7, 0.2, 0.2, 0.9]
                };
                push_solid_rect(elements, layout.close_all, scale, close_col);

                // Background
                push_solid_rect(elements, layout.main_rect, scale, [0.06, 0.07, 0.1, 0.95]);
            }
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
    rect: Rectangle<i32, Logical>,
    scale: Scale<f64>,
    thickness: i32,
    color: [f32; 4],
) where
    R: Renderer + ImportAll + ImportMem,
    R::TextureId: Clone + 'static,
{
    let borders = [
        Rectangle::new(rect.loc, Size::new(rect.size.w, thickness)),
        Rectangle::new(
            Point::new(rect.loc.x, rect.loc.y + rect.size.h - thickness),
            Size::new(rect.size.w, thickness),
        ),
        Rectangle::new(rect.loc, Size::new(thickness, rect.size.h)),
        Rectangle::new(
            Point::new(rect.loc.x + rect.size.w - thickness, rect.loc.y),
            Size::new(thickness, rect.size.h),
        ),
    ];
    for b in borders {
        push_solid_rect(elements, b, scale, color);
    }
}
