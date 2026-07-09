# YUYUDHAN patch v2: per-profile whitelist advertising, gated on split health

Vendored RMK at upstream rev `b98204928e9a8532064ce99add1f6c4c554e08c9`
(https://github.com/HaoboGu/rmk), pristine except for the patch below.
Wired via `[patch."https://github.com/HaoboGu/rmk"]` in the repo-root
`Cargo.toml`; patching `rmk` alone cascades to its `../rmk-macro`,
`../rmk-types`, `../rmk-config` path-deps inside this tree.

## Problem 1 (v1): profile stealing

Stock RMK advertises ONE identical, undirected, unfiltered BLE identity for
every profile (`rmk/src/ble/mod.rs`, `ConnectableScannableUndirected`, default
`AdvFilterPolicy::Unfiltered`). A profile only selects which LTK is loaded.
With two bonded computers in range, whichever host answers the advertisement
first wins the connection — switching to profile 0 lets the profile-1 computer
steal the slot, encryption fails against the wrong LTK, and the keyboard sits
in a dead unencrypted connection while the OLED shows `~ P0` indefinitely.
Upstream master (checked 2026-07-02) has no fix.

## Problem 2 (v2): v1 starved the split link

The nRF controller has ONE filter accept list shared by every radio role.
Two code paths program it:

- v1's filtered advertising (`Peripheral::set_filter_accept_list` +
  `AdvFilterPolicy::FilterConnAndScan`) — holds the list for the whole
  advertising window (up to 300 s) whenever the active profile is bonded.
- The split central's peripheral (re)connect
  (`split/ble/central.rs`, `Central::connect` with
  `scan_config.filter_accept_list = &[peripheral_addr]`) — trouble-host
  hard-requires the list (`ConfigFilterAcceptListIsEmpty` otherwise).

Per the BLE spec the accept list cannot be modified while in use by a
filtered advertiser, so every split reconnect attempt fails while v1's
filtered advertising is up. Symptom: switching BLE profiles (especially
rapidly, or to a bonded-but-absent host) leaves the right half blinking ✗
until the left half is power-cycled. v1 also touched the list on the OPEN
path (`set_filter_accept_list(&[])` to clear it), churning it on every
profile switch even for unbonded profiles.

## Fix (v2)

`rmk/src/ble/mod.rs`:

- `advertise()` takes `peer: Option<Address>`. `Some` → the peer is loaded
  into the accept list and advertising runs `FilterConnAndScan` (invisible
  and unconnectable to every other host). `None` → advertising is
  `Unfiltered` and the accept list is NOT touched at all (zero HCI calls
  against it — no clear), so the open path can never contend with the split.
- The connection loop computes `peer` from
  `ProfileManager::active_bond_info()`, guarded on `identity.irk.is_some()`
  (a bond without an IRK could belong to an RPA-rotating host the controller
  cannot resolve) AND on `all_peripherals_connected()`: filtered advertising
  only runs while every split peripheral holds a live link. While any
  peripheral is down, advertising is open and the accept list stays free for
  the reconnect.
- The loop `select3`s on a new `SPLIT_LINK_CHANGED` arm: any split link
  transition aborts the in-flight advertise and restarts it with the gate
  recomputed. The signal is `reset()` BEFORE the gate is read so a transition
  in between latches a wake instead of being lost.
- `BleTransport` bounds widened with
  `ControllerCmdSync<LeClearFilterAcceptList> + ControllerCmdSync<LeAddDeviceToFilterAcceptList>`
  (both implemented by nrf-sdc's SoftDevice Controller).

`rmk/src/split/ble/central.rs`:

- `CONNECTED_PERIPHERALS: AtomicU8` + `SPLIT_LINK_CHANGED: Signal` +
  `all_peripherals_connected()`. Increment/decrement strictly paired inside
  the successful-connection arm (decrement saturating, so a spurious call can
  never wrap the counter into a permanently-"healthy" gate). Boot default 0 →
  gate closed → open advertising at boot, so the split wins the initial
  connect race before filtering ever arms.

Rotating private addresses (macOS/Windows) are handled by trouble-host, which
auto-syncs the controller resolving list from bond IRKs with
`PrivacyMode::Device` whenever bond information changes.

Behavioral rules this introduces:

- **A bonded profile is invisible to other hosts while the split is whole.**
  To pair a profile to a new computer, clear it first (User6 / ESC+O on the
  MEDIA layer) — that returns the profile to open advertising.
- **While the right half is reconnecting (a few seconds, rare), the keyboard
  advertises open** — briefly stealable by another bonded host in range.
  That is the price of not starving the split link.

## Escape hatch

If the split still misbehaves after v2: in `rmk/src/ble/mod.rs`'s connection
loop, force `let peer: Option<Address> = None;` (delete the
`active_bond_info()` chain). That is stock-RMK open advertising — split is
guaranteed reliable, profile stealing returns.

## Files touched

- `rmk/src/ble/mod.rs`
- `rmk/src/split/ble/central.rs`

Full delta in `YUYUDHAN_PATCH.patch`.

## Regenerate the diff

```sh
P=~/.cargo/git/checkouts/rmk-cd9707f7f2031ce2/b982049
diff -u $P/rmk/src/ble/mod.rs vendor/rmk/rmk/src/ble/mod.rs > vendor/rmk/YUYUDHAN_PATCH.patch
diff -u $P/rmk/src/split/ble/central.rs vendor/rmk/rmk/src/split/ble/central.rs >> vendor/rmk/YUYUDHAN_PATCH.patch
```

## Vendor layout note

Only `rmk/`, `rmk-types/`, `rmk-macro/`, `rmk-config/`, licenses and the root
`README.md` are vendored (no docs/, examples/, .github/). `rmk/README.md` is a
symlink to `../README.md`, hence the root copy — `rmk/src/lib.rs` includes it
via `#![doc = include_str!("../README.md")]`.
