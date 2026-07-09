//! Custom central (left-half) OLED renderer — 128×32 SSD1306, drawn landscape
//! (`rotation = 0`), styled after the **stock ZMK built-in status screen**
//! that this keyboard ran on ZMK: plain, utilitarian, three pieces of truth.
//!
//!   top-left      output status — "USB" (wired output) or "BT0".."BT3"
//!                 (+ "~" while that profile is advertising / not connected)
//!   top-right     battery percentage (bolt glyph while on USB power — the
//!                 rail measurement reads the USB supply then, so the number
//!                 is stale)
//!   bottom-left   layer name, bold
//!
//! The right half's battery is on the right half's own screen (parix.rs).
//! No `render_interval` — all draws are event-driven, zero idle cost.
//! If text reads the wrong way for your mount: rotation = 0 → 180.

use core::fmt::Write as _;

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle};
use rmk::display::{DisplayRenderer, RenderContext};
use rmk::heapless::String;
use rmk::types::battery::BatteryStatus;
use rmk::types::ble::BleState;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{FontRenderer, fonts};

use crate::layer_names::{DISPLAY_OFF_LAYER, LAYER_NAMES};

/// True while USB power (VBUS) is present on this half.
fn usb_powered() -> bool {
    embassy_nrf::pac::POWER.usbregstatus().read().vbusdetect()
}

/// Central (left-half) OLED renderer — landscape 128×32, ZMK-style.
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

        let meta_font  = FontRenderer::new::<fonts::u8g2_font_6x12_tr>();       // right column
        let layer_font = FontRenderer::new::<fonts::u8g2_font_logisoso16_tr>(); // hero
        let on = FontColor::Transparent(BinaryColor::On);
        let stroke1 = PrimitiveStyle::with_stroke(BinaryColor::On, 1);

        // ── Top-left: output status (ZMK's "USB / BT n") ─────────────────────
        let mut out_buf: String<8> = String::new();
        match ctx.ble_status.state {
            BleState::Inactive => {
                let _ = write!(out_buf, "USB");
            }
            BleState::Connected => {
                let _ = write!(out_buf, "BT{}", ctx.ble_status.profile);
            }
            BleState::Advertising => {
                let _ = write!(out_buf, "BT{} ~", ctx.ble_status.profile);
            }
        }
        meta_font
            .render_aligned(out_buf.as_str(), Point::new(126, 3),
                            VerticalPosition::Top, HorizontalAlignment::Right, on, display)
            .ok();

        // ── Top-right: battery percentage ────────────────────────────────────
        let mut bat_buf: String<6> = String::new();
        match *ctx.battery {
            BatteryStatus::Available { level: Some(pct), .. } => {
                let _ = write!(bat_buf, "{}%", pct);
            }
            _ => {
                let _ = write!(bat_buf, "--");
            }
        }
        meta_font
            .render_aligned(bat_buf.as_str(), Point::new(126, 29),
                            VerticalPosition::Bottom, HorizontalAlignment::Right, on, display)
            .ok();
        if usb_powered() {
            // Small bolt to the left of the percentage: "on mains, number stale".
            let bx = 126 - (bat_buf.len() as i32) * 6 - 9;
            Line::new(Point::new(bx + 3, 19), Point::new(bx, 23)).into_styled(stroke1).draw(display).ok();
            Line::new(Point::new(bx, 23), Point::new(bx + 4, 23)).into_styled(stroke1).draw(display).ok();
            Line::new(Point::new(bx + 4, 23), Point::new(bx + 1, 28)).into_styled(stroke1).draw(display).ok();
        }

        // ── Bottom-left: layer name, bold (ZMK's layer widget) ───────────────
        let mut layer_buf: String<8> = String::new();
        let layer_str: &str = if (ctx.layer as usize) < LAYER_NAMES.len() {
            LAYER_NAMES[ctx.layer as usize]
        } else {
            let _ = write!(layer_buf, "L{}", ctx.layer);
            &layer_buf
        };
        layer_font
            .render_aligned(layer_str, Point::new(2, 16),
                            VerticalPosition::Center, HorizontalAlignment::Left, on, display)
            .ok();
    }
}
