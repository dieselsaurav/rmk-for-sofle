//! Custom central (left-half) OLED renderer — 128×32 SSD1306, mounted **portrait**
//! (short/32 px edge faces the user). `rotation = 90` in keyboard.toml makes the
//! DrawTarget 32 wide × 128 tall; all coordinates below are in that portrait frame.
//!
//! Band map (y = 0 at the far end from the user, y = 127 nearest the user):
//!
//!   rows   2-25   OM glyph (Devanagari ॐ, 30×24 px, static — event-driven only)
//!   rows  27-44   Layer name (adaptive font; inverted box when not BASE)
//!   rows  48-57   WPM  "W{n}" FONT_6X10
//!   rows  61-75   BLE profile "P{n}" FONT_9X15_BOLD (centred)
//!   rows  77-84   BLE state "~" / "USB" FONT_5X8 (blank when connected)
//!   rows  87-95   Battery gauge (outline + proportional fill; blinks at <20%)
//!   rows  98-112  Battery number FONT_9X15_BOLD (centred; no "%" — gauge supplies it)
//!   rows 117-123  Firmware version "y{ver}" FONT_4X6 (centred; from VERSION file)
//!
//! No SCAG modifier display — removed per user request.
//! No `render_interval` — all draws are event-driven (layer/BLE/WPM/battery changes),
//! so idle battery cost is zero.
//!
//! Rotation contingency: if content appears upside-down on hardware, change
//! `rotation = 90` → `rotation = 270` in config/keyboard.toml — layout unchanged.

use core::fmt::Write as _;

use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::mono_font::ascii::{FONT_4X6, FONT_5X8, FONT_6X10, FONT_9X15_BOLD};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::{Baseline, Text};
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::heapless::String;
use rmk::types::battery::BatteryStatus;
use rmk::types::ble::BleState;
use crate::layer_names::{DISPLAY_OFF_LAYER, LAYER_NAMES};
use crate::bitmaps::{OM, draw_page_format_frame};

/// Firmware version, embedded at build time from the repo-root `VERSION` file
/// (see build.rs). Shown on the central OLED as `y{FW_VERSION}` so you can read
/// off exactly which build is flashed.
const FW_VERSION: &str = env!("YUYUDHAN_FW_VERSION");

/// Central (left-half) OLED renderer — portrait 32×128 canvas.
/// Stateless: no animation tick needed; all redraws are event-driven.
#[derive(Default)]
pub struct StatusRenderer;

impl DisplayRenderer<BinaryColor> for StatusRenderer {
    fn render<D: DrawTarget<Color = BinaryColor>>(&mut self, ctx: &RenderContext, display: &mut D) {
        display.clear(BinaryColor::Off).ok();
        if ctx.sleeping {
            return;
        }
        if ctx.layer == DISPLAY_OFF_LAYER {
            return;
        }

        let fill_on   = PrimitiveStyle::with_fill(BinaryColor::On);
        let big       = MonoTextStyle::new(&FONT_9X15_BOLD, BinaryColor::On);
        let big_inv   = MonoTextStyle::new(&FONT_9X15_BOLD, BinaryColor::Off);
        let medium    = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let medium_inv= MonoTextStyle::new(&FONT_6X10, BinaryColor::Off);
        let tiny      = MonoTextStyle::new(&FONT_5X8,  BinaryColor::On);

        // ── 1. OM glyph (rows 2–25) ──────────────────────────────────────────
        // Static — no animation on the central half (event-driven only, zero idle cost).
        draw_page_format_frame(display, &OM, 30, 1, 2);

        // ── 2. Layer name (rows 27–44) ────────────────────────────────────────
        // Font choice: ≤3-char names (NAV, NUM, SYM, FUN) fit FONT_9X15_BOLD in
        // 32 px (max 27 px); 4-5 char names (BASE, MEDIA, MOUSE) use FONT_6X10
        // (max 30 px).  Non-BASE layers get an inverted full-width highlight bar.
        let mut layer_buf: String<8> = String::new();
        let layer_str: &str = if (ctx.layer as usize) < LAYER_NAMES.len() {
            LAYER_NAMES[ctx.layer as usize]
        } else {
            let _ = write!(layer_buf, "L{}", ctx.layer);
            &layer_buf
        };

        let is_base   = ctx.layer == 0;
        let use_big   = layer_str.len() <= 3;
        // char advance: FONT_9X15_BOLD = 9 px, FONT_6X10 = 6 px.
        let char_adv  = if use_big { 9i32 } else { 6i32 };
        // Vertical centre of the 18-px band (rows 27–44):
        //   big  font 15 px → top at 27 + (18-15)/2 = 28.
        //   medium font 10 px → top at 27 + (18-10)/2 = 31 (rounding up).
        let text_y    = if use_big { 28i32 } else { 31i32 };
        let text_x    = ((32 - layer_str.len() as i32 * char_adv) / 2).max(0);

        if !is_base {
            // Full-width inverted bar covering the layer name band.
            Rectangle::new(Point::new(0, 27), Size::new(32, 18))
                .into_styled(fill_on)
                .draw(display)
                .ok();
        }

        if use_big {
            Text::with_baseline(layer_str, Point::new(text_x, text_y),
                                if is_base { big } else { big_inv }, Baseline::Top)
                .draw(display).ok();
        } else {
            Text::with_baseline(layer_str, Point::new(text_x, text_y),
                                if is_base { medium } else { medium_inv }, Baseline::Top)
                .draw(display).ok();
        }

        // ── 3. WPM (rows 48–57) ──────────────────────────────────────────────
        // "W{n}" in FONT_6X10.  "W200" = 4 chars × 6 = 24 px → x = 4, centred.
        let mut wpm_buf: String<8> = String::new();
        let _ = write!(wpm_buf, "W{}", ctx.wpm);
        let wpm_x = ((32 - wpm_buf.len() as i32 * 6) / 2).max(0);
        Text::with_baseline(&wpm_buf, Point::new(wpm_x, 48), medium, Baseline::Top)
            .draw(display)
            .ok();

        // ── 4. BLE profile (rows 61–75) ──────────────────────────────────────
        // "P0"–"P3" always visible; FONT_9X15_BOLD (15 px), centred.
        // 2 chars × 9 px = 18 px → x = (32 - 18) / 2 = 7.
        let mut profile_buf: String<4> = String::new();
        let _ = write!(profile_buf, "P{}", ctx.ble_status.profile);
        let profile_x = ((32 - profile_buf.len() as i32 * 9) / 2).max(0);
        Text::with_baseline(&profile_buf, Point::new(profile_x, 61), big, Baseline::Top)
            .draw(display)
            .ok();

        // ── 5. BLE state indicator (rows 77–84) ──────────────────────────────
        // Blank when connected (normal operation — no noise).
        // "~"   when advertising (keyboard is searching for the profile's host).
        // "USB" when inactive/wired (USB mode; profile memorised and ready).
        let state_str: &str = match ctx.ble_status.state {
            BleState::Connected   => "",
            BleState::Advertising => "~",
            BleState::Inactive    => "USB",
        };
        if !state_str.is_empty() {
            // FONT_5X8 advance = 5 px/char.
            let state_x = ((32 - state_str.len() as i32 * 5) / 2).max(0);
            Text::with_baseline(state_str, Point::new(state_x, 77), tiny, Baseline::Top)
                .draw(display)
                .ok();
        }

        // ── 6. Battery gauge (rows 87–95) ────────────────────────────────────
        // Outline 28×9 at (2, 87); nub 2×3 at (30, 90); fill from (4, 89).
        Rectangle::new(Point::new(2, 87), Size::new(28, 9))
            .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
            .draw(display)
            .ok();
        Rectangle::new(Point::new(30, 90), Size::new(2, 3))
            .into_styled(fill_on)
            .draw(display)
            .ok();

        let mut bat_num: String<4> = String::new();
        match *ctx.battery {
            BatteryStatus::Available { level: Some(pct), .. } => {
                // Fill width proportional to percentage (max 24 px inside the outline).
                // No blink here: central has no tick counter (event-driven only).
                // Low battery (<20%): show fill at all times; the small number is warning enough.
                let fill_w = (pct as u32 * 24 / 100).max(1);
                Rectangle::new(Point::new(4, 89), Size::new(fill_w, 5))
                    .into_styled(fill_on)
                    .draw(display)
                    .ok();
                let _ = write!(bat_num, "{}", pct);
            }
            BatteryStatus::Available { level: None, .. } => {
                // Level reading unavailable — show "--" as the number.
                let _ = write!(bat_num, "--");
            }
            BatteryStatus::Unavailable => {
                let _ = write!(bat_num, "?");
            }
        }

        // ── 7. Battery number (rows 98–112) ──────────────────────────────────
        // FONT_9X15_BOLD (15 px), centred.  No "%" — gauge provides context;
        // "100%" (36 px) would overflow the 32 px width.
        if !bat_num.is_empty() {
            let bat_x = ((32 - bat_num.len() as i32 * 9) / 2).max(0);
            Text::with_baseline(&bat_num, Point::new(bat_x, 98), big, Baseline::Top)
                .draw(display)
                .ok();
        }

        // ── 8. Firmware version (rows 117–123) ───────────────────────────────
        // "y{version}" in FONT_4X6 (4 px/char), centred. FONT_4X6 (not 5X8) so
        // multi-digit bumps still fit: "y0.0.100" = 8 chars × 4 px = 32 px, the
        // full width. Tells you which build is flashed.
        let ver_style = MonoTextStyle::new(&FONT_4X6, BinaryColor::On);
        let mut ver_buf: String<12> = String::new();
        let _ = write!(ver_buf, "y{}", FW_VERSION);
        let ver_x = ((32 - ver_buf.len() as i32 * 4) / 2).max(0);
        Text::with_baseline(&ver_buf, Point::new(ver_x, 117), ver_style, Baseline::Top)
            .draw(display)
            .ok();
    }
}
