# octo hub

One Linux window for the Attack Shark X6 mouse, the ZUOYA GMK70 keyboard
and the M-Vave Tank-G guitar pedal. The home screen shows the three side by
side; click one to open its pages.

- **Mouse**: DPI stages, polling, sensor, lighting, button bindings, sleep.
  Uses [remora](../remora) as a library and shares its remembered profile
  (`~/.config/remora/mouse.json`), since the X6 cannot report its settings.
- **Keyboard**: keymap, knob, lighting, macros, key tester, Windows/Mac
  switch, VIA layout files. Uses [hidra](../hidra) as a library and its
  definitions and mode files under `~/.config/hidra`.
- **Pedal**: the sounding preset's six blocks edited live, save / save to,
  bypass, the 36-preset grid, backup and restore. The protocol is ported from
  TabJam's pedal panel. The MIDI port (`/dev/snd/midiCnD0`) is opened only
  while the pedal screen is shown, so TabJam can still use the pedal. Backups
  go to `~/.local/share/octo-hub/tank-g/`; one is taken before the first save
  of every session.

## Build and install

```sh
cargo build --release
install -Dm755 target/release/octo-hub ~/.local/bin/octo-hub
install -Dm644 packaging/octo-hub.desktop ~/.local/share/applications/octo-hub.desktop
install -Dm644 packaging/octo-hub.svg ~/.local/share/icons/hicolor/scalable/apps/octo-hub.svg
```

hidra and remora must sit next to this folder (`../hidra`, `../remora`).

## Flags

```sh
octo-hub --demo                       # in-memory mouse and keyboard, simulated pedal
octo-hub --screen keyboard --tab macros
octo-hub --demo --screen pedal --shot pedal.png   # saves one frame and quits
```

`OCTO_PEDAL_PORT=/dev/snd/midiC4D0` forces the pedal port.
