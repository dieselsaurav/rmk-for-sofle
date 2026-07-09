//! WS2812 RGB driver — event processor driving the Sofle's LED chain from the
//! nRF52840 PWM engine (DMA sequence at 800 kHz), since RMK has no native
//! WS2812/underglow support yet.
//!
//! Wiring (from the ZMK led_strip config): 36 LEDs per half (6 underglow +
//! 30 per-key), data on P0.06 (pro_micro D1), GRB wire order.
//!
//! Behavior:
//!   - static color per active layer (dim palette — battery first)
//!   - LEDs off while the half is sleeping
//!   - layer 7 (DISPOFF, TG(7) on MEDIA) doubles as the RGB kill switch
//!
//! Note: WS2812s draw ~0.5-1 mA each even when dark (no power mosfet on the
//! Sofle, same as running ZMK with EXT_POWER=n). Real "off" is the power
//! switch.

use embassy_nrf::Peri;
use embassy_nrf::gpio::Pin as GpioPin;
use embassy_nrf::pwm::{
    Config, Instance, Prescaler, SequenceConfig, SequencePwm, SingleSequenceMode, SingleSequencer,
};
use rmk::event::{LayerChangeEvent, SleepStateEvent};
use rmk::macros::processor;

/// Number of WS2812 LEDs on one half (6 underglow + 30 per-key).
const NUM_LEDS: usize = 36;
/// PWM ticks (16 MHz) per WS2812 bit: 20 ticks = 1.25 us = 800 kHz.
const BIT_TICKS: u16 = 20;
/// Compare value for a WS2812 "0" bit (~0.375 us high). Bit 15 = polarity.
const DUTY_ZERO: u16 = 0x8000 | 6;
/// Compare value for a WS2812 "1" bit (~0.8125 us high).
const DUTY_ONE: u16 = 0x8000 | 13;
/// All-low slots appended for the >50 us WS2812 reset latch.
const RESET_SLOTS: usize = 40;
const BUF_LEN: usize = NUM_LEDS * 24 + RESET_SLOTS;

/// (r, g, b) per layer — deliberately dim (~12% peak vs ZMK's BRT_MAX 60%)
/// to keep battery draw sane. Index = layer number.
const LAYER_COLORS: [(u8, u8, u8); 8] = [
    (28, 18, 8),  // 0 BASE  — warm white
    (0, 10, 30),  // 1 NAV   — blue
    (0, 28, 6),   // 2 NUM   — green
    (22, 0, 28),  // 3 MEDIA — purple
    (30, 12, 0),  // 4 SYM   — orange
    (30, 2, 2),   // 5 FUN   — red
    (0, 22, 22),  // 6 MOUSE — cyan
    (0, 0, 0),    // 7 DISPOFF — off (RGB kill switch, same toggle as OLEDs)
];

#[processor(subscribe = [LayerChangeEvent, SleepStateEvent])]
pub struct RgbProcessor {
    pwm: SequencePwm<'static>,
    buf: [u16; BUF_LEN],
    layer: u8,
    sleeping: bool,
}

impl RgbProcessor {
    pub fn new(pwm: Peri<'static, impl Instance>, pin: Peri<'static, impl GpioPin>) -> Self {
        let mut config = Config::default();
        config.prescaler = Prescaler::Div1; // 16 MHz
        config.max_duty = BIT_TICKS;
        let pwm = SequencePwm::new_1ch(pwm, pin, config).expect("ws2812 pwm init");

        Self {
            pwm,
            buf: [0x8000; BUF_LEN],
            layer: 0,
            sleeping: false,
        }
    }

    /// Encode the current color into the PWM duty buffer (GRB wire order).
    fn fill_buffer(&mut self) {
        let (r, g, b) = if self.sleeping {
            (0, 0, 0)
        } else {
            LAYER_COLORS[(self.layer as usize).min(LAYER_COLORS.len() - 1)]
        };
        let mut i = 0;
        for _ in 0..NUM_LEDS {
            for byte in [g, r, b] {
                for bit in (0..8).rev() {
                    self.buf[i] = if (byte >> bit) & 1 == 1 { DUTY_ONE } else { DUTY_ZERO };
                    i += 1;
                }
            }
        }
        // Trailing slots stay at 0% duty for the reset latch.
        for slot in self.buf[NUM_LEDS * 24..].iter_mut() {
            *slot = 0x8000;
        }
    }

    /// Push the buffer out to the strip and wait for the DMA sequence to end
    /// (36 LEDs x 30 us + 50 us reset ~= 1.2 ms).
    pub async fn show(&mut self) {
        self.fill_buffer();
        let sequencer = SingleSequencer::new(&mut self.pwm, &self.buf, SequenceConfig::default());
        if sequencer.start(SingleSequenceMode::Times(1)).is_ok() {
            embassy_time::Timer::after_millis(2).await;
        }
    }

    async fn on_layer_change_event(&mut self, event: LayerChangeEvent) {
        if event.0 != self.layer {
            self.layer = event.0;
            self.show().await;
        }
    }

    async fn on_sleep_state_event(&mut self, event: SleepStateEvent) {
        if event.0 != self.sleeping {
            self.sleeping = event.0;
            self.show().await;
        }
    }
}
