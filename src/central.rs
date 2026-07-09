#![no_main]
#![no_std]

use rmk::macros::rmk_central;

mod bitmaps;
mod status;
mod layer_names;

#[rmk_central]
mod keyboard_central {}
