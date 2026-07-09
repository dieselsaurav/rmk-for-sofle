//! Parix right-half OLED renderer — faithful port of the ZMK custom status
//! screen (`custom_status_screen.c` in dieselsaurav/zmk-for-sofle):
//!
//!   +----------+---------+---------+
//!   | link     |         | battery |
//!   +----------+  [P]    +---------+
//!   |          |         |         |
//!   +----------+---------+---------+
//!
//! The P keycap (extracted from the PARIX logo) sits centred and every few
//! seconds suffers a short burst of digital glitch: row shifts, static,
//! inversion, page smears — the same four effects, same xorshift PRNG and
//! same timing as the ZMK implementation.
//!
//! Landscape 128×32 (`rotation = 0` in keyboard.toml, unlike the portrait
//! left half). `render_interval = 150` drives the glitch animation, matching
//! the ZMK 150 ms LVGL timer.

use core::fmt::Write as _;

use embassy_time::{Duration, Instant};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle};
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::heapless::String;
use rmk::types::battery::BatteryStatus;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{FontRenderer, fonts};

use crate::bitmaps::draw_page_format_frame;
use crate::layer_names::DISPLAY_OFF_LAYER;

/// P keycap extracted from the PARIX logo — 24 cols × 32 rows (4 SSD1306
/// pages), page format. Byte-identical to the ZMK `raw_p_keycap`.
const P_KEYCAP: [u8; 96] = [
    // page 0 (rows 0-7)
    0, 0, 192, 224, 112, 48, 48, 48, 48, 48, 48, 48,
    48, 48, 48, 48, 48, 48, 48, 48, 112, 224, 192, 0,
    // page 1 (rows 8-15)
    0, 0, 255, 255, 0, 0, 0, 0, 0, 254, 66, 66,
    66, 66, 102, 60, 0, 0, 0, 0, 0, 255, 255, 0,
    // page 2 (rows 16-23)
    0, 0, 255, 255, 128, 0, 0, 0, 0, 7, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 128, 255, 255, 0,
    // page 3 (rows 24-31)
    0, 0, 7, 15, 31, 31, 31, 31, 31, 31, 31, 31,
    31, 31, 31, 31, 31, 31, 31, 31, 31, 15, 7, 0,
];

const P_WIDTH: usize = 24;
/// X where the 24-px-wide keycap starts (left-edge hero, mirroring the
/// layer name position on the left half's screen).
const P_X: i32 = 2;

// ── Glitch tuning — identical values to the ZMK implementation ──────────────
const GLITCH_INTERVAL_MIN_MS: u64 = 2000;
const GLITCH_INTERVAL_RANGE_MS: u64 = 6000;
const GLITCH_INITIAL_DELAY_MS: u64 = 3000;
const GLITCH_FRAMES_MIN: u8 = 2;
const GLITCH_FRAMES_RANGE: u8 = 4;

/// Right-half OLED renderer — landscape 128×32 canvas.
pub struct ParixRenderer {
    /// xorshift16 PRNG state (seed 42, like the ZMK version).
    seed: u16,
    /// Glitch frames still to draw in the current burst.
    frames_left: u8,
    /// When the next glitch burst may start.
    next_glitch_at: Option<Instant>,
    /// Animation clock — +1 per render; drives the ✗ blink.
    tick: u8,
}

impl Default for ParixRenderer {
    fn default() -> Self {
        Self {
            seed: 42,
            frames_left: 0,
            next_glitch_at: None,
            tick: 0,
        }
    }
}

impl ParixRenderer {
    fn rand(&mut self) -> u16 {
        let mut s = self.seed;
        s ^= s << 7;
        s ^= s >> 9;
        s ^= s << 8;
        self.seed = s;
        s
    }

    /// One glitch frame over the page-format buffer — port of `apply_glitch`.
    fn apply_glitch(&mut self, buf: &mut [u8; 96]) {
        match self.rand() % 4 {
            // 0: shift one page horizontally by -3..=3
            0 => {
                let page = (self.rand() % 4) as usize;
                let shift = (self.rand() % 7) as i32 - 3;
                let row = &mut buf[page * P_WIDTH..(page + 1) * P_WIDTH];
                if shift > 0 {
                    for c in (shift as usize..P_WIDTH).rev() {
                        row[c] = row[c - shift as usize];
                    }
                } else if shift < 0 {
                    let s = (-shift) as usize;
                    for c in 0..P_WIDTH - s {
                        row[c] = row[c + s];
                    }
                }
            }
            // 1: random static over a small span
            1 => {
                let page = (self.rand() % 4) as usize;
                let col = (self.rand() % (P_WIDTH as u16 - 4)) as usize;
                let w = 3 + (self.rand() % 6) as usize;
                for c in col..(col + w).min(P_WIDTH) {
                    buf[page * P_WIDTH + c] = self.rand() as u8;
                }
            }
            // 2: invert a span
            2 => {
                let page = (self.rand() % 4) as usize;
                let col = (self.rand() % (P_WIDTH as u16 - 4)) as usize;
                let w = 4 + (self.rand() % 10) as usize;
                for c in col..(col + w).min(P_WIDTH) {
                    buf[page * P_WIDTH + c] = !buf[page * P_WIDTH + c];
                }
            }
            // 3: smear one page's columns onto another
            _ => {
                let src = (self.rand() % 4) as usize;
                let dst = (src + 1 + (self.rand() % 3) as usize) % 4;
                let col = (self.rand() % (P_WIDTH as u16 / 2)) as usize;
                let w = P_WIDTH / 3 + (self.rand() % (P_WIDTH as u16 / 3)) as usize;
                for c in col..(col + w).min(P_WIDTH) {
                    buf[dst * P_WIDTH + c] = buf[src * P_WIDTH + c];
                }
            }
        }
    }
}

impl DisplayRenderer<BinaryColor> for ParixRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();
        if ctx.sleeping {
            return;
        }
        if ctx.layer == DISPLAY_OFF_LAYER {
            return;
        }

        self.tick = self.tick.wrapping_add(1);
        let now = Instant::now();

        // ── P keycap with glitch bursts ───────────────────────────────────────
        let next = *self
            .next_glitch_at
            .get_or_insert(now + Duration::from_millis(GLITCH_INITIAL_DELAY_MS));

        let mut buf = P_KEYCAP;
        if self.frames_left > 0 {
            self.frames_left -= 1;
            self.apply_glitch(&mut buf);
        } else if now >= next {
            // schedule a new burst and the one after it
            self.frames_left = GLITCH_FRAMES_MIN + (self.rand() % GLITCH_FRAMES_RANGE as u16) as u8;
            self.next_glitch_at = Some(
                now + Duration::from_millis(
                    GLITCH_INTERVAL_MIN_MS + (self.rand() as u64 % GLITCH_INTERVAL_RANGE_MS),
                ),
            );
        }
        draw_page_format_frame(display, &buf, P_WIDTH, P_X, 0);

        let stroke1 = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
        let stroke2 = PrimitiveStyle::with_stroke(BinaryColor::On, 2);
        let meta_font = FontRenderer::new::<fonts::u8g2_font_6x12_tr>();
        let on = FontColor::Transparent(BinaryColor::On);

        // ── Split-link status — small, top-right corner ──────────────────────
        if ctx.central_connected {
            // ✓ check
            Line::new(Point::new(117, 7), Point::new(120, 10)).into_styled(stroke1).draw(display).ok();
            Line::new(Point::new(120, 10), Point::new(125, 3)).into_styled(stroke1).draw(display).ok();
        } else if self.tick % 2 == 0 {
            // ✗ cross, blinking
            Line::new(Point::new(117, 3), Point::new(125, 11)).into_styled(stroke2).draw(display).ok();
            Line::new(Point::new(125, 3), Point::new(117, 11)).into_styled(stroke2).draw(display).ok();
        }

        // ── Battery % — right, blinks below 20% ──────────────────────────────
        let mut text: String<5> = String::new();
        let (pct, low) = match *ctx.battery {
            BatteryStatus::Available { level: Some(p), .. } => (Some(p), p < 20),
            _ => (None, false),
        };
        match pct {
            Some(p) => {
                let _ = write!(&mut text, "{}%", p);
            }
            None => {
                let _ = write!(&mut text, "--");
            }
        }
        if !(low && self.tick % 2 == 0) {
            meta_font
                .render_aligned(text.as_str(), Point::new(126, 29),
                                VerticalPosition::Bottom, HorizontalAlignment::Right, on, display)
                .ok();
            // Lightning bolt while this half is on USB power — the rail
            // measurement reads the USB supply then, so the number is stale.
            if embassy_nrf::pac::POWER.usbregstatus().read().vbusdetect() {
                let bx = 126 - (text.len() as i32) * 6 - 9;
                Line::new(Point::new(bx + 3, 19), Point::new(bx, 23)).into_styled(stroke1).draw(display).ok();
                Line::new(Point::new(bx, 23), Point::new(bx + 4, 23)).into_styled(stroke1).draw(display).ok();
                Line::new(Point::new(bx + 4, 23), Point::new(bx + 1, 28)).into_styled(stroke1).draw(display).ok();
            }
        }
    }
}
