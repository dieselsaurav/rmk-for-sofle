#![no_main]
#![no_std]

use rmk::macros::rmk_peripheral;

mod bitmaps;
mod trishul;
mod layer_names;

#[rmk_peripheral(id = 0)]
mod keyboard_peripheral {}
