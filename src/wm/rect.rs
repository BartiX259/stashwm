use resvg::tiny_skia::{Pixmap, Transform as SkiaTransform};
use resvg::usvg::{Options, Tree};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
use smithay::utils::{Logical, Point, Size, Transform};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RenderedRect {
    pub buffer: MemoryRenderBuffer,
    pub padding: i32,
}

impl RenderedRect {
    pub fn location(&self, inner_loc: Point<i32, Logical>) -> Point<i32, Logical> {
        Point::new(inner_loc.x - self.padding, inner_loc.y - self.padding)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct RectCacheKey {
    width: i32,
    height: i32,
    radius: u16,
    fill_color: [u8; 4],
    border_width: u16,
    border_color: [u8; 4],
}

pub struct RectRenderer {
    cache: HashMap<RectCacheKey, RenderedRect>,
}

impl std::fmt::Debug for RectRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RectRenderer").finish()
    }
}

impl RectRenderer {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn render(
        &mut self,
        size: Size<i32, Logical>,
        radius: f32,
        fill_color: [f32; 4],
        border_width: f32,
        border_color: [f32; 4],
    ) -> RenderedRect {
        let w = size.w.max(1);
        let h = size.h.max(1);
        // Compensate for curvature thinning on thin rounded borders
        let border_width = if radius > 0.0 && border_width > 0.0 && border_width <= 1.0 {
            1.5
        } else {
            border_width
        };

        let byte_fill = to_byte_color(fill_color);
        let byte_border = to_byte_color(border_color);

        let key = RectCacheKey {
            width: w,
            height: h,
            radius: (radius.max(0.0) * 10.0).round() as u16,
            fill_color: byte_fill,
            border_width: (border_width.max(0.0) * 10.0).round() as u16,
            border_color: byte_border,
        };

        self.cache
            .entry(key)
            .or_insert_with(|| {
                let has_border = border_width > 0.0 && byte_border[3] > 0;
                let pad = if has_border { border_width.ceil() as i32 } else { 0 };

                let total_w = (w + pad * 2) as u32;
                let total_h = (h + pad * 2) as u32;

                // Fill color
                let fill_svg = if byte_fill[3] > 0 {
                    format!(
                        r#"<rect x="{pad}" y="{pad}" width="{w}" height="{h}" rx="{radius}" fill="{}" />"#,
                        to_css_rgba(byte_fill)
                    )
                } else {
                    String::new()
                };

                // Border
                let stroke_svg = if has_border {
                    let offset = border_width / 2.0;
                    format!(
                        r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" fill="none" stroke="{}" stroke-width="{}" />"#,
                        pad as f32 - offset,
                        pad as f32 - offset,
                        w as f32 + border_width,
                        h as f32 + border_width,
                        (radius + offset).max(0.0),
                        to_css_rgba(byte_border),
                        border_width
                    )
                } else {
                    String::new()
                };

                let svg = format!(
                    r#"<svg width="{total_w}" height="{total_h}" xmlns="http://www.w3.org/2000/svg">{fill_svg}{stroke_svg}</svg>"#
                );

                let tree = Tree::from_str(&svg, &Options::default()).expect("Failed to parse SVG");
                let mut pixmap = Pixmap::new(total_w, total_h).expect("Failed to create pixmap");
                resvg::render(&tree, SkiaTransform::identity(), &mut pixmap.as_mut());

                let pixels = pixmap.take();
                let buffer = MemoryRenderBuffer::from_slice(
                    &pixels,
                    Fourcc::Abgr8888,
                    (total_w as i32, total_h as i32),
                    1,
                    Transform::Normal,
                    None,
                );

                RenderedRect { buffer, padding: pad }
            })
            .clone()
    }

    pub fn render_solid(
        &mut self,
        size: Size<i32, Logical>,
        radius: f32,
        fill_color: [f32; 4],
    ) -> RenderedRect {
        self.render(size, radius, fill_color, 0.0, [0.0, 0.0, 0.0, 0.0])
    }

    pub fn render_outline(
        &mut self,
        size: Size<i32, Logical>,
        radius: f32,
        border_width: f32,
        border_color: [f32; 4],
    ) -> RenderedRect {
        self.render(
            size,
            radius,
            [0.0, 0.0, 0.0, 0.0],
            border_width,
            border_color,
        )
    }
}

fn to_byte_color(c: [f32; 4]) -> [u8; 4] {
    [
        (c[0].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[1].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[2].clamp(0.0, 1.0) * 255.0).round() as u8,
        (c[3].clamp(0.0, 1.0) * 255.0).round() as u8,
    ]
}

fn to_css_rgba(c: [u8; 4]) -> String {
    format!(
        "rgba({}, {}, {}, {:.3})",
        c[0],
        c[1],
        c[2],
        c[3] as f32 / 255.0
    )
}
