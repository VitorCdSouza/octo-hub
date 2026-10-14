# Handoff: Octo Hub — one Linux app for the mouse, keyboard and pedal

## Overview
Octo Hub merges three native Linux configurators into one window, Logitech-hub style:

- **remora**: Attack Shark X6 wireless mouse (Rust + egui, hidraw)
- **hidra**: ZUOYA GMK70 VIA/QMK keyboard (Rust + egui, hidraw)
- **TabJam's pedal panel**: M-Vave Tank-G guitar pedal (C# + Avalonia, ALSA rawmidi). Only the pedal part moves over; the tab/practice features stay in TabJam.

The home screen shows the three devices side by side. Click one to open its pages. The main goal is **intuitive and uncluttered**: every label sits next to the thing it describes, there are no stray status labels, and controls appear only when they apply.

## About the design files
`Octo Hub.dc.html` is a **design reference built in HTML**: a clickable prototype that shows the intended look and behaviour. It is not production code. Rebuild it in the real app. The natural target is **Rust + egui**, so remora and hidra can become modules of one binary that shares `theme.rs` and `widgets.rs`. The pedal protocol (`Pedal/TankG.cs`, `MvaveFrame.cs`, `AlsaRawMidi.cs`, `TankGPreset.cs`) has to be ported from C# to Rust.

To open the prototype, put `Octo Hub.dc.html` and `support.js` in the same folder and open the HTML in a browser (it needs internet for Google Fonts). The Tweaks panel has three props: `startScreen` (home/mouse/keyboard/pedal), `homeLayout` (cards/rows) and `pedalConnected` (bool).

## Fidelity
**High-fidelity.** Colours, type sizes, spacing, radii and copy are final. Keep egui's flat look; the values below map directly onto `Visuals`/`Style`.

---

## Design tokens

One theme for every device, with one accent.

| Token | Hex | Use |
|---|---|---|
| BG | `#131416` | window background, text fields, segmented track |
| TOPBAR | `#17181b` | top bar; home card icon area `#16171a` |
| PANEL | `#1a1b1e` | cards |
| CARD_ALT | `#1e2023` | DPI stage tiles, preset cells, menus, dialog |
| HI | `#26282c` | buttons, knob face, hover background `#222428` |
| LINE | `#2e3035` | button/input borders, slider track |
| LINE_SOFT | `#26282c` / `#25272b` / `#232529` | card border / row divider / top bar border |
| TEXT | `#e8e6e1` | body text |
| TEXT_SOFT | `#c4c2bc` | secondary values, unselected segment text |
| TEXT_DIM | `#8e8f93` | hints, captions, labels |
| TEXT_FAINT | `#6b6d72` / `#55575c` | disabled, empty |
| ACCENT | `#f0914e` | selection, primary button, active segment, knob arc |
| ACCENT_DIM | `#8c5128` | slider trailing fill, selected DPI preset chip |
| ACCENT_TINT | `#2a201a` | background of the selected list row or tile |
| ACCENT_TEXT | `#2a1406` | text on accent fills |
| OK | `#8cc79b` | connected dot, battery, "saved" toast, tester "worked" |
| DANGER_TEXT / LINE | `#f0a596` / `#6e3a32` | destructive outline buttons |
| ERR | `#e8685a` | confirm-reset button fill |
| CAP_ALPHA / CAP_MOD | `#e6e2da` / `#a8a49c` | keycaps; legend `#2a2825` |
| CAP_GHOST | `#2a2c30`, legend `#6b6d72` | ▽ transparent keys on layers 1–3 |
| Mouse drawing line | `#55585e`, dividers `#3a3d42` | line art |

**Type.** Ubuntu (egui's default proportional font) at weights 300/400/500, and JetBrains Mono for numbers and codes (egui's default monospace, Hack, works too).
- Body 14; buttons and segments 13–13.5; captions 12–12.5
- Card heading 15/500; page-level number 22–40/300
- DPI values in mono 20/600; preset slot name in mono 30/600

**Spacing and shape**

| Element | Value |
|---|---|
| Card | padding 18×20, radius 12, border 1px LINE_SOFT; 16px gap between cards |
| Button | height 32 (30 when compact), padding 0 14, radius 7 |
| Segmented control | track radius 8, padding 3, gap 2; item padding 5×13, radius 6 |
| List row | padding 9×12, radius 8; selected = 1px ACCENT border + ACCENT_TINT fill + ACCENT text |
| Switch | 40×22 track (34×20 small), 16px knob, on = ACCENT, off = `#3a3c42` |
| Slider | 4px track LINE, trailing fill ACCENT_DIM, 18px TEXT-coloured thumb |
| Content area | max width 1240, padding 22×24 |
| Top bar | height 56 |

**Shadows**: the dropdown and dialog only (`0 12px 32px rgba(0,0,0,.45)`).

---

## Global layout

### Top bar (56px, TOPBAR, bottom border `#232529`)
From left to right:
1. **Logo button**: 22px ACCENT circle with two 4px dark "eyes", then "octo hub" 15/500. Clicking it goes **home**. Hover background `#222428`.
2. In a device view only:
   - A 1px divider.
   - The **device switcher**: 24px icon, device name, ▼. It opens a 300px dropdown listing all devices with a one-line status and a connection dot; click outside to close.
   - The **tabs**: a pill group (BG track, `#232529` border, radius 9). The active tab has `#2a2c30` fill and TEXT; the others are TEXT_DIM.
3. A spacer, then a **device status** on the right:
   - Mouse: stage colour dot + "1200 DPI" (click → DPI tab) and a battery glyph + "70%" in OK (click → Power).
   - Keyboard: "Layout Windows" chip (click → Device tab).
   - Pedal: "Playing **1A**", plus "· bypassed" when bypassed.

The top bar items never shrink or wrap (`flex:none`, `nowrap`).

### Saved toast
The confirmation is a toast, not a status bar. It floats at the bottom centre, 22px from the edge: CARD_ALT fill, LINE border, radius 9, OK text such as "✓ Saved to the mouse". It stays for 1.8 s and is replaced by each new write. **There is no bottom status bar** (the old "demo / in-memory mouse" footer is gone).

---

## Screens

### Home
- Heading "Devices" 24/300; the area has max width 1060 and 48px top padding.
- **Cards**: grid `repeat(auto-fill, minmax(290px,1fr))`, gap 18. Each card:
  - **Icon area**: 168px tall, `#16171a`, line icon at 1.7× scale, TEXT_SOFT.
  - **Name** 16, with the kind ("Mouse", "Keyboard", "Guitar pedal") under it in 13 TEXT_DIM.
  - **Headline stat**:
    - Mouse: stage colour dot + "**1200** DPI" in mono 22/600, and a battery on the right.
    - Keyboard: "**Windows** layout".
    - Pedal: "**1A** preset", or "bypassed" in place of "preset".
  - **Footer**: separated by a 1px line. A 7px dot (OK = connected, `#55575c` = not) and a connection line: "Connected · 2.4 GHz receiver", "Connected · USB", "Connected · USB MIDI".
  - **Hover**: border `#4a4c52`, lift 2px.
  - **Disconnected**: the card is at 60% opacity, the headline is "Not connected" (Ubuntu 300) and the footer is "Not found. Plug it in over USB".
- **Rows layout** (alternative): one card holding a grid of rows (56px icon | name+kind | stat | connection | ›).

**Line icons** (48×48, 2px stroke, currentColor). Draw them with the egui painter.
- **Mouse**: 28×40 rounded rect, radius 14, at (10,4); scroll tick 2×9 at (23,10).
- **Keyboard**: 46×27 rect, radius 5; two 32×3 bars and one 18×3 spacebar.
- **Pedal**: 42×32 rect, radius 6; three 5px knob dots on top and two 10px ring footswitches.

### Mouse: DPI & sensor
1. **DPI stages card**
   - Heading plus the hint "The DPI button on the mouse steps through these. Click one to use and edit it."
   - Right side: − "6 stages" + (1–8 stages).
   - **Stage tiles**: grid `minmax(112px,1fr)`. Each tile has a 4px colour bar, the number, an "in use" tag in ACCENT on the active one, and the DPI value in mono 20/600. The active tile has a 1.5px ACCENT border and ACCENT_TINT fill.
   - **Editor** under a divider, a 2-column grid with a 90px label column:
     - "DPI": a 300px **logarithmic** slider (50–26000, snaps to 50) and a value box "1200 DPI".
     - Under it, aligned with the slider: preset chips 400 / 800 / 1200 / 1600 / 2400 / 3200 / 6400 / 12800 (selected chip ACCENT_DIM).
     - "Colour": 10 swatches, 22px circles: `#ff3b30 #ff7a1a #ffb01a #ffe81a #34e05a #2fe0e0 #2f7bff #4a3aff #e040fb #ffffff`. The selected one gets a 2px gap ring and a 2px TEXT ring.
   - Clicking a tile makes it the active stage **and** the one being edited (one concept, not two).
2. **Settings card**: rows of label + one-line hint on the left, control on the right, with dividers between rows.
   - Polling rate: segmented 125/250/500/1000 Hz.
   - Lift-off distance: segmented 1 mm / 2 mm.
   - Angle snapping: switch.
   - Ripple control: switch.

### Mouse: Lighting
Two columns: a 240–300px effect list and a preview card.
- **Effect list**: name on the left, short note on the right in 12 DIM: DPI colour (stage colour), DPI breathing (stage colour), Steady (one colour), Breathing (one colour), Neon (cycles colours), Colour breathing (cycles colours), Off.
- **Preview**: the mouse line drawing (see below). **Only the LED lights up**: the X6 has a single LED below the DPI buttons. The 8px LED dot gets the effect colour with a glow (`0 0 6px 1px c, 0 0 16px 4px c/33%`). Animations:
  - Breathing: opacity 0.18↔1, ease-in-out.
  - Neon: hue-rotate 360°, linear.
  - Colour breathing: both at once.
  - The duration comes from speed 1–5 = 6 / 4.5 / 3 / 2 / 1.2 s.
- **Controls shown only when relevant**:
  - Colour swatches (label above, centred) for Steady and Breathing.
  - The note "Lights up in the colour of the DPI stage in use." plus an ACCENT link "Change stage colours" (→ DPI tab) for the DPI effects.
  - A Speed 1–5 segmented control for the animated effects.

### Mouse drawing (Lighting and Buttons)
This is a simple line drawing, not an illustration.
- **Body**: a pill. 124×208 on Lighting, 136×228 on Buttons. Radius = half the width, 2px `#55585e` stroke.
- **Click buttons**: split by a vertical line from the top down to a horizontal line at **y≈38%** (y=81 on Lighting, y=87 on Buttons). Dividers are 2px `#3a3d42`.
- **Scroll wheel**: centred pill, 20×34 (Lighting) / 22×38 (Buttons), top 14/16.
- **DPI housing**: a rounded rect (radius 6) **from just under the wheel down past the LED**. 24×80 at top 44 on Lighting; 26×84 at top 50 on Buttons.
  - Two **portrait** DPI buttons inside it: 14×18 at y 62 and 84 (Lighting); 16×20 at y 66 and 90 (Buttons).
  - **The click-button divider runs through the gap between the two DPI buttons.**
  - The **LED**: an 8px dot centred under the lower DPI button (y 110 on Lighting, y 118 on Buttons).
- Everything is centred on the body's centre line.
- **Buttons page only**: two side buttons stick out of the left flank (9×32, radius 5 on the outside), starting just under the click-button line (y 118 and 156).

### Mouse: Buttons
Two columns: a 260–320px left card and the right side.
- **Left card**: the drawing with every part clickable, and a list of the 7 buttons under it: Left, Right, Wheel, Forward, Back, DPI +, DPI −. Each row shows the current action on the right (ACCENT if selected, DIM if default).
  - Selected part: the main buttons fill `rgba(240,145,78,.18)` with an ACCENT stroke; the small parts fill solid ACCENT.
  - The list and the drawing select the same thing.
- **Right card**:
  - Header: "**Wheel** does **Middle click**" (the action in ACCENT 15) and a "Default" button, dimmed when the action is already the default.
  - A category segmented control: Mouse / DPI / Media / Keys / Off, then the action list (the default one is tagged "default").
  - **Left button**: only the Mouse category, a few click actions, and the hint "The left button only takes clicks, so you can't lose it."
  - **Keys** category: a "Record a shortcut" button, which becomes "● Listening…" (ACCENT fill). It captures the next key combo, e.g. "Ctrl+Shift+T".
- **Click response card**: the debounce slider, 0–20 ms.

### Mouse: Power
- **Battery card**: big "70%" (40/300), a 6px OK bar, and "Awake. The receiver reports in steps of 10%."
- **"Settings on the mouse" card**: the text "The X6 can't report what it holds, so octo hub keeps what it last wrote. If another app changed the mouse, write everything again." Then a primary "Write everything" button and a danger "Factory reset…" button.
  - "Factory reset…" opens an inline confirmation box (`#241a18` fill, DANGER_LINE border): "Put every setting back to how the mouse shipped?" with Reset (ERR fill) and Cancel.
- **Sleep card** (right): "Sleep after" slider 0.5–30 min (step 0.5) and "Deep sleep after" slider 1–60 min.

### Keyboard: Keymap
- **Layer selector**: segmented "0 Base · 1 Fn · 2 Mac · 3 Mac Fn", with a one-line hint for the chosen layer on the right.
- **Board card**: the GMK70 layout drawn in key units; the coordinates are in `KEYS` in the prototype JS. The board is 17.75u × 5u, scales with the width (max 1100), and each key has 2.5px padding.
  - Caps have radius 7, a `inset 0 -4px 0 rgba(0,0,0,.2)` bottom lip, and 15px legends (12.5 for two-line legends, 11.5 for long ones).
  - Accent cap (Quote) and the knob (a circle) are filled ACCENT.
  - On layers 1–3, ▽ keys are drawn as ghost caps, so you can see at once what the Fn layer changes.
  - Selected key: a ring `0 0 0 2px PANEL, 0 0 0 4px ACCENT`.
- **Below the board**, two columns:
  - **Keycodes**:
    - Categories Basic / Media / Layers / Macro / Lighting and a "Search keycodes" field.
    - Small caps 40px tall; the grid is at 55% opacity when no key is selected.
    - A free-text QMK field ("Any QMK keycode, e.g. LT(1,KC_SPC)") with Apply.
  - **Detail** (300px):
    - Empty: "Click a key or the knob to change what it does."
    - Selected: a 60px ACCENT preview cap, the code in mono 15/600 ACCENT, a description, and ▽ Transparent / Clear buttons.
    - Knob: Press / Turn left ↺ / Turn right ↻ rows.
    - Footer: "Picking a keycode moves on to the next key. Esc stops."
- Picking a keycode writes it and moves the selection to the next key (VIA behaviour). For the knob it moves to the next encoder slot.

### Keyboard: Lighting
- The same board, with each cap showing an underglow in the effect colour (preview only).
- Three cards below:
  - **Effect list** with a filter field (VIA effects 0–45, grouped).
  - **Backlight**: Brightness slider, and Effect speed only for animated effects. When the effect is All Off: "The lights are off. Pick an effect to turn them on."
  - **Colour**: a rainbow hue strip slider plus 8 swatches. At 45% opacity with "This effect picks its own colours." when the effect ignores colour.

### Keyboard: Macros
- **Left**: list M0–M15 with a mono preview, "empty" when unset.
- **Editor card**:
  - Macro name in ACCENT, a byte-budget bar "28 / 3322 bytes", and a mono textarea.
  - Under it: **Add** chips that append syntax: Tap a key `{KC_A}`, Keys together `{KC_LCTL,KC_C}`, Hold `{+KC_LSFT}`, Release `{-KC_LSFT}`, Wait `{250}`.
- **Preview card**: "When you press M0, the keyboard will", then step chips (`#352519` fill: kind label `#d7a27a` 11.5, value mono `#f3d2b8`).
- Revert and Save to keyboard buttons, both dimmed until something changed.

### Keyboard: Key tester
- The board with all caps cream; held keys turn ACCENT and keys that worked turn OK. It listens to real key events and clicks.
- Progress card: "12 of 72 keys work", an OK bar, legend chips (held / worked / not yet), "Start over".
- Event card: matrix `[row,col]`, key, down/up (down in ACCENT).
- Empty text: "Press keys on the keyboard. Each switch is read straight from the matrix, so a dead one shows even if the keymap hides it."

### Keyboard: Device
Grid of cards, `minmax(340px,1fr)`:
1. **Windows / Mac layout**
   - Before takeover: an explanation and a primary "Let octo hub switch it…" button, which opens the dialog.
   - After takeover: a Windows | Mac segmented control, the note "Switching writes the chosen layout into layer 0…" and a link "Give the switch back to the keyboard".
2. **Layout file**: Save layout file… / Load layout file… (VIA format).
3. **Copy a layer**: [layer] over [layer] Copy.
4. **Info rows**: Definition, VIA protocol, Matrix, Layers, Encoders, Macros. Below them: Blink lights, plus danger buttons Reset keymap… and Bootloader….

**Dialog**: a 500px modal over a dark overlay (`rgba(10,10,12,.62)`). Title "Let octo hub switch Windows / Mac?", two short paragraphs, Cancel and Take over (primary).

### Pedal: Tone
- **Header card**:
  - ‹ **1A** › stepper (slot name mono 30/600 ACCENT; clicking it opens Presets).
  - Clean state: "Saved on the pedal. Turn a knob to hear it change."
  - Changed state: "● Changed, not saved", then **Save to 1A** (primary), a "Save to…" slot select, and Revert.
  - Bypass switch on the right. While bypassed, a line of text explains it and the chain is at 35% opacity and can't be edited.
- **Signal chain**: block cards in a wrapping row, gap 10, **no arrows between them**. Order: Gate, Amp, Mod, Delay, Reverb, Cab.
  - Each card: name, a small on/off switch, and a body at 40% opacity when the block is off.
  - Flex basis: Amp `2 1 380px` (min 340); Mod `1 1 230px` (min 230); the others `1 1 170px` (min 170).
  - **Amp**: Model 1–9 stepper, knobs Gain / Treble / Middle / Bass / Volume (0–127).
  - **Mod**: Chorus / Phaser / Tremolo segmented control, Speed knob.
  - **Delay**: Mix, Time. **Reverb**: Decay, Mix. **Gate**: Threshold. **Cab**: IR slot 1–8 stepper.
  - Ranges come from `TankGBlock.All`.
- **Knob**: a 50px dial.
  - It has a 270° conic arc starting at −135° (ACCENT, or `#55575c` when the block is off) over a LINE ring, a HI face and a white 2×10 pointer.
  - The value shows under it in mono 13 and the label in 12 DIM.
  - **Drag vertically** (160px = full range); **double-click** restores the stored value. Turning a knob sends the value live; it is only stored on Save.

### Pedal: Presets
- **Grid**: 9 banks (columns 1–9) × 4 letters (rows A–D), each cell 58px tall.
  - Each cell shows the slot name and six dots for gate / amp / mod / delay / reverb / cab, lit when the block is on.
  - The playing slot: ACCENT border and tint. Click a cell to load it.
  - Intro line: "Click a preset to play it. The footswitches step through the same grid."
- **Backup card**: "All 36 presets in one file. A copy is also kept before the first save of every session." Buttons: Back up all presets… / Restore from a backup….

### Pedal: not connected
A centred empty state: a dim pedal icon, "Tank-G not found", "Plug it in over USB. If it's passed through to a virtual machine, give it back to Linux first.", and buttons Try again (primary) / Use a simulated pedal (maps to `SimulatedTankG`).

---

## Interactions and state
- **Navigation**: `screen` = home | mouse | keyboard | pedal, and each device remembers its own tab. The logo goes home; the switcher jumps between devices directly.
- **Writes**:
  - Mouse and keyboard write as you edit. Sliders send once the hand stops (remora uses `SETTLE` = 140 ms). Each write shows the toast.
  - Pedal edits are live but **not stored** until Save; "dirty" = the live block differs from the stored preset.
- **Mouse**: keep remora's "remembered profile" model (`~/.config/remora/mouse.json`). The DPI stage reported by the hardware buttons updates the top bar and the tiles.
- **Keyboard**: as in hidra. Mode takeover follows `modes.rs` (the TO keys become ▽ and switching writes layer 0).
- **Pedal**: as in TabJam:
  - Poll the active preset register and the live block once a second.
  - Bypass reads all zero, so keep the preset's values on screen.
  - Back up all 36 presets before the first write of a session.
  - Never write the `A5 5A` header and never send Factory Restore.
- **Hover**: rows and buttons `#222428`; cards border `#4a4c52`.
- **Transitions**: 120–150 ms on background, left and border. egui animates switches with `animate_bool`.

## Assets
None. The icons and the mouse drawing are simple shapes; draw them with egui's `Painter` (rects, circles, lines) at the sizes above. Fonts: Ubuntu + JetBrains Mono (or egui's defaults).

## Files
- `Octo Hub.dc.html`: the prototype. The logic class at the bottom holds the data: the `KEYS` layout, the `CODES` keycode catalogue, `KFX` effects, `ACTIONS` button actions, `BLOCKS` pedal blocks, `parseMacro`.
- `support.js`: the runtime needed to open the prototype in a browser.
- Source apps for behaviour and protocols:
  - `uploads/remora-design/`
  - `uploads/hidra-design/`
  - `uploads/tabjam/TabJam/Pedal/` and `PedalPanel.axaml(.cs)`
