//! Shared SSD1306 page-format bitmaps and the pixel-drawing helper used by
//! both halves' OLED renderers.
//!
//! Each binary (`central`, `peripheral`) declares `mod bitmaps;` in its entry
//! point so this module is compiled into both.  `TRISHUL` lives in
//! `src/trishul.rs` (peripheral only) to avoid dead-code in the central build.

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;

/// Devanagari Om (U+0950), 30 cols × 24 rows (3 SSD1306 pages), page format.
/// Rasterised from `/System/Library/Fonts/Kohinoor.ttc` (index 0), U+0950,
/// 400 px render, `ImageFilter.MaxFilter(5)`, LANCZOS thumbnail to 30 px,
/// threshold 90.  Verified by round-trip ASCII preview.
///
/// Regeneration recipe (run once, paste bytes):
/// ```python
/// from PIL import Image, ImageDraw, ImageFont, ImageFilter
/// f = ImageFont.truetype("/System/Library/Fonts/Kohinoor.ttc", 400, index=0)
/// img = Image.new("L", (1600, 1600), 0); d = ImageDraw.Draw(img)
/// d.text((800, 800), "\u0950", font=f, fill=255, anchor="mm")
/// g = img.crop(img.getbbox()).filter(ImageFilter.MaxFilter(5))
/// g.thumbnail((30, 30), Image.LANCZOS)
/// out = Image.new("L", (30, 30), 0)
/// out.paste(g, ((30 - g.width) // 2, (30 - g.height) // 2))
/// out = out.point(lambda p: 255 if p >= 90 else 0)
/// # pack: data[page*30+col] = OR of (1<<(y%8)) for set pixels in that page
/// ```
///
/// Hardware gate: if ॐ reads poorly on the physical panel, tweak threshold
/// (try 60–110) and re-embed.  If still muddy, remove the OM draw call and
/// the bindu halo in both renderers and shift other elements up.
pub const OM: [u8; 90] = [
    // page 0 (rows 0-7)
    0, 0, 128, 192, 192, 192, 192, 192, 128, 0, 0, 48, 112, 240, 224, 192, 199, 199, 199, 194, 224, 240, 112, 48, 0, 0, 0, 0, 0, 0,
    // page 1 (rows 8-15)
    0, 1, 3, 3, 193, 193, 225, 243, 255, 191, 30, 0, 0, 0, 0, 1, 1, 1, 1, 129, 224, 240, 120, 60, 28, 28, 28, 248, 248, 224,
    // page 2 (rows 16-23)
    48, 120, 240, 224, 224, 192, 192, 225, 243, 127, 63, 62, 56, 112, 112, 112, 120, 60, 30, 31, 63, 121, 112, 96, 96, 112, 112, 127, 63, 15,
];

/// Draw an SSD1306 page-format bitmap onto an embedded-graphics DrawTarget.
///
/// `data[page * cols + col]`, bit `b` → pixel at
/// `(col + offset_x, page*8 + b + offset_y)`.  LSB (bit 0) = top row of the
/// page.  Copied verbatim from rmk `display/renderers/logo.rs` (private there).
pub fn draw_page_format_frame<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    data: &[u8],
    cols: usize,
    offset_x: i32,
    offset_y: i32,
) {
    let pages = data.len() / cols;
    for page in 0..pages {
        for col in 0..cols {
            let byte = data[page * cols + col];
            if byte == 0 {
                continue;
            }
            for bit in 0..8u32 {
                if byte & (1 << bit) != 0 {
                    let x = col as i32 + offset_x;
                    let y = page as i32 * 8 + bit as i32 + offset_y;
                    Pixel(Point::new(x, y), BinaryColor::On).draw(display).ok();
                }
            }
        }
    }
}
