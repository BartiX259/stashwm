use fontdue::{Font, FontSettings};
use smithay::backend::{allocator::Fourcc, renderer::element::memory::MemoryRenderBuffer};
use smithay::utils::{Logical, Point, Rectangle, Size, Transform};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RenderedText {
    pub buffer: MemoryRenderBuffer,
    pub size: Size<i32, Logical>,
}

impl RenderedText {
    pub fn centered_in(&self, container: Rectangle<i32, Logical>) -> Point<i32, Logical> {
        Point::new(
            container.loc.x + (container.size.w - self.size.w) / 2,
            container.loc.y + (container.size.h - self.size.h) / 2,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TextCacheKey {
    text: String,
    size_pt: u32,
    color: [u8; 4],
}

pub struct TextRenderer {
    font: Option<Font>,
    fallback_empty: RenderedText,
    cache: HashMap<TextCacheKey, RenderedText>,
}

impl std::fmt::Debug for TextRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextRenderer").finish()
    }
}

impl TextRenderer {
    pub fn new() -> Self {
        let system_paths = [
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/dejavu-sans-fonts/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/noto/NotoSans-Regular.ttf",
            "/usr/share/fonts/liberation-sans/LiberationSans-Regular.ttf",
        ];

        let font = system_paths
            .iter()
            .find_map(|path| std::fs::read(path).ok())
            .and_then(|bytes| Font::from_bytes(bytes, FontSettings::default()).ok());

        let fallback_empty = RenderedText {
            buffer: MemoryRenderBuffer::from_slice(
                &[0, 0, 0, 0],
                Fourcc::Abgr8888,
                (1, 1),
                1,
                Transform::Normal,
                None,
            ),
            size: Size::new(1, 1),
        };

        Self {
            font,
            fallback_empty,
            cache: HashMap::new(),
        }
    }

    pub fn render(&mut self, text: &str, size_pt: f32, color: [f32; 4]) -> RenderedText {
        let Some(font) = &self.font else {
            return self.fallback_empty.clone();
        };
        if self.cache.len() > 256 {
            self.cache.clear();
        }

        let byte_color = [
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
            (color[3] * 255.0) as u8,
        ];

        let key = TextCacheKey {
            text: text.to_string(),
            size_pt: (size_pt * 10.0) as u32,
            color: byte_color,
        };

        let cached = self.cache.entry(key).or_insert_with(|| {
            let line_metrics = font.horizontal_line_metrics(size_pt);
            let (ascent, descent) = if let Some(m) = line_metrics {
                (m.ascent, m.descent)
            } else {
                (size_pt, -size_pt * 0.25)
            };

            let max_height = (ascent - descent).ceil() as i32;
            let baseline = ascent.round() as i32;

            let mut total_width = 0;
            for ch in text.chars() {
                let metrics = font.metrics(ch, size_pt);
                total_width += metrics.advance_width.ceil() as i32;
            }

            let total_width = total_width.max(1);
            let max_height = max_height.max(1);
            let mut canvas = vec![0u8; (total_width * max_height * 4) as usize];

            let mut x_offset = 0;
            for ch in text.chars() {
                let (metrics, bitmap) = font.rasterize(ch, size_pt);

                for row in 0..metrics.height {
                    for col in 0..metrics.width {
                        let alpha = bitmap[row * metrics.width + col];
                        if alpha == 0 {
                            continue;
                        }

                        let y = baseline - (metrics.ymin + metrics.height as i32) + row as i32;
                        let x = x_offset + metrics.xmin + col as i32;

                        if x >= 0 && x < total_width && y >= 0 && y < max_height {
                            let idx = ((y * total_width + x) * 4) as usize;
                            let a_factor = (alpha as f32 / 255.0) * (byte_color[3] as f32 / 255.0);

                            canvas[idx] = (byte_color[0] as f32 * a_factor) as u8;
                            canvas[idx + 1] = (byte_color[1] as f32 * a_factor) as u8;
                            canvas[idx + 2] = (byte_color[2] as f32 * a_factor) as u8;
                            canvas[idx + 3] = (255.0 * a_factor) as u8;
                        }
                    }
                }
                x_offset += metrics.advance_width.ceil() as i32;
            }

            let buffer = MemoryRenderBuffer::from_slice(
                &canvas,
                Fourcc::Abgr8888,
                (total_width, max_height),
                1,
                Transform::Normal,
                None,
            );

            RenderedText {
                buffer,
                size: Size::new(total_width, max_height),
            }
        });

        cached.clone()
    }
}
