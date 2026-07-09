# sofle-rmk

[RMK](https://github.com/HaoboGu/rmk) (Rust) firmware for the **PandaKB Sofle** —
nice!nano (nRF52840), BLE split, rotary encoders, SSD1306 OLED, Vial.

Ported from my ZMK config ([dieselsaurav/zmk-for-sofle](https://github.com/dieselsaurav/zmk-for-sofle)).
Project scaffold based on [jakoritarleite/sofle-rmk](https://github.com/jakoritarleite/sofle-rmk) (same hardware).

## Layout

Miryoku-style: QWERTY base with GACS home row mods, 7 layers
(BASE / NAV / NUM / MEDIA / SYM / FUN / MOUSE), combos (caps word on F+J, etc.),
encoders (volume / page scroll). Left half is the **central**, right is the **peripheral**.

Keymap is editable live with [Vial](https://get.vial.today/) — over USB or BLE.

### BLE keys (MEDIA layer, Vial "User" keycodes)

| Key | Function |
|---|---|
| User0–User7 | select BLE profile 0–7 (ZMK `BT_SEL`) |
| User8 / User9 | next / previous profile |
| User10 | clear current profile bond (ZMK `BT_CLR`) |
| User11 | toggle USB/BLE output (ZMK `OUT_TOG`) |

## Building

Cloud: push to GitHub — the workflow uploads `sofle-rmk-central.uf2` and
`sofle-rmk-peripheral.uf2` as artifacts.

Local:

```sh
rustup target add thumbv7em-none-eabihf
cargo install cargo-make
cargo make uf2          # outputs build/sofle-rmk-{central,peripheral}.uf2
```

## Flashing

1. Double-tap reset on the **left** half → drag `sofle-rmk-central.uf2` onto the `NICENANO` drive.
2. Same on the **right** half with `sofle-rmk-peripheral.uf2`.

## ⚠️ Going back to ZMK

RMK ≥ 0.7 replaces the Nordic SoftDevice BLE stack with its own. **To return to
ZMK later you must first re-flash the
[nice!nano bootloader](https://nicekeyboards.com/docs/nice-nano/troubleshooting#my-nicenano-seems-to-be-acting-up-and-i-want-to-re-flash-the-bootloader)**,
then flash ZMK as usual. Keep this in mind before flashing both halves.

## Not ported (RMK gaps)

- **RGB underglow** — RMK lighting support is still maturing; the ZMK `rgb_ug` keys were dropped.
- **nice!view** — unsupported; this port drives the stock SSD1306 OLED (layer,
  USB/BLE, peripheral status). With a nice!view fitted the keyboard works but the screen stays blank.
- **`ext_power` toggle, deep sleep** — not yet in RMK.
- ZMK tier-3 combos (cut, vertical symbol combos) trimmed to fit RMK's default combo slots.
