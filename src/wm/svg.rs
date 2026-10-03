use resvg::tiny_skia::{Pixmap, Transform as SkiaTransform};
use resvg::usvg::{Options, Tree};
use smithay::utils::{Logical, Size};
use smithay::{
    backend::allocator::Fourcc, backend::renderer::element::memory::MemoryRenderBuffer,
    utils::Transform,
};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Icon {
    Close,
    Minimize,
    Maximize,
}

impl Icon {
    pub fn svg_data(self) -> &'static str {
        match self {
            Icon::Close => include_str!("../../resources/icons/close.svg"),
            Icon::Minimize => include_str!("../../resources/icons/minimize.svg"),
            Icon::Maximize => include_str!("../../resources/icons/maximize.svg"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SvgCacheKey {
    icon: Icon,
    width: i32,
    height: i32,
    color: [u8; 4],
}

pub struct SvgRenderer {
    cache: HashMap<SvgCacheKey, MemoryRenderBuffer>,
}
impl std::fmt::Debug for SvgRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SvgRenderer").finish()
    }
}

impl SvgRenderer {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn render_svg(
        &mut self,
        icon: Icon,
        size: Size<i32, Logical>,
        color: [f32; 4],
    ) -> MemoryRenderBuffer {
        let byte_color = [
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            (color[3] * 255.0) as u8,
        ];

        let svg_data = icon.svg_data();
        let key = SvgCacheKey {
            icon,
            width: size.w,
            height: size.h,
            color: byte_color,
        };

        let cached = self.cache.entry(key).or_insert_with(|| {
            let opt = Options::default();
            let tree = Tree::from_str(svg_data, &opt).expect("Failed to parse SVG string");

            let mut pixmap =
                Pixmap::new(size.w as u32, size.h as u32).expect("Failed to create RGBA pixmap");

            let svg_size = tree.size();
            let sx = size.w as f32 / svg_size.width();
            let sy = size.h as f32 / svg_size.height();
            let transform = SkiaTransform::from_scale(sx, sy);

            resvg::render(&tree, transform, &mut pixmap.as_mut());

            let mut pixels = pixmap.take();
            for chunk in pixels.chunks_exact_mut(4) {
                let a = chunk[3] as f32 / 255.0;
                chunk[0] = (color[0] * 255.0 * a) as u8;
                chunk[1] = (color[1] * 255.0 * a) as u8;
                chunk[2] = (color[2] * 255.0 * a) as u8;
                chunk[3] = (color[3] * 255.0 * a) as u8;
            }

            MemoryRenderBuffer::from_slice(
                &pixels,
                Fourcc::Argb8888,
                (size.w, size.h),
                1,
                Transform::Normal,
                None,
            )
        });

        cached.clone()
    }
}
