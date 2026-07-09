#![no_main]
#![no_std]

use rmk::macros::rmk_peripheral;

mod bitmaps;
mod layer_names;
mod parix;
mod rgb;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {
    use crate::rgb::RgbProcessor;

    /// WS2812 RGB chain on the right half (36 LEDs, data on P0.06).
    #[register_processor(event)]
    fn rgb_underglow() -> RgbProcessor {
        let mut rgb = RgbProcessor::new(p.PWM0, p.P0_06);
        rgb.show().await; // paint the BASE color at boot
        rgb
    }
}
