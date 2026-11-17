# castanea launcher: UI design

Clean and matte, like a well-made native settings app: flat solid surfaces, 1 px hairlines where things need to be
apart, one muted accent used sparingly, one type family, few icons. No gradients, glow, shadows, tinted cards, icon
bubbles or illustrations. Every colour, size and duration is a token in `crates/ui/src/theme.rs`; pages build on the
components in `crates/ui/src/widgets/`. Text is Inter (OFL), icons are Phosphor line icons (MIT), see `third_party/`.
The project's own copper font ([The brand font](#the-brand-font)) sets the wordmark, page titles and the Play label.
Screenshots of every page: [screenshots/](screenshots/).

## Frame

The window uses the system's frame (title bar, snapping and resizing as the platform does them). Inside it:

```
+----------------+---------------------------------------------------------------+
| > Play         |                                                               |  sidebar 184 wide, BASE,
|   Mods         |                     page content                              |  a hairline on its right
|   Game output •|                     (32 margin at the sides, 24 at the top)   |  • = the game is running
|   Settings     |                                                               |
|                |                                                               |
|                |                                                               |
| Lade 0.3.0 43 %|                                                               |  a quiet line, only for a
|   About        |                                                               |  download or offline mode
+----------------+---------------------------------------------------------------+
```

Sidebar items are a line icon and a label, 32 high. The selected one is RAISED with semibold text; the others are
muted and fade to HOVER under the pointer. Each item is a tab for screen readers. Ctrl+1 … Ctrl+5 open the pages in
this order, F5 plays, Ctrl+F searches mods, Esc closes a question or leaves the mod details.

## Play (home)

```
                                 CASTANEA                         brand font, 52 pt line height

                          +------------------------+
                          |          PLAY          |              240 x 48, dark plate, accent outline
                          +------------------------+
                          ======------------------                2 px progress, only while downloading
                 castanea 0.2.0  ·  up to date                    12 pt muted, one line
                              Game output                         a quiet link, once the game has run
```

The block sits a little above the middle of the page. The status line says one of:

| State | Line |
|---|---|
| checked, nothing newer | `castanea 0.2.0 · up to date` |
| a newer release | `castanea 0.2.0 · update available, downloading 0.3.0 · 43 %` (Play still starts 0.2.0) |
| downloaded while the game ran | `castanea 0.2.0 · 0.3.0 is downloaded and starts next time` |
| swapping | `castanea 0.2.0 · installing 0.3.0…` (Play is off for that moment) |
| offline | `castanea 0.2.0 · offline`; with nothing installed `Offline. Go online once to download the game.` |
| check failed | `castanea 0.2.0 · could not check for updates` in the danger colour, the error as tooltip, and a Try again link |
| local build folder | `Local build 0.9.0-dev` |
| nothing installed, no release | `No game release is published yet.` |

While the game runs the button is Stop (a secondary button: hairline outline, no fill) with the running time in a
fixed-width slot after the label, so the label does not move as the seconds change.

## Mods

```
 Mods                                                   [ Installed | Browse ]
 [Import zip…] [Import folder…] [Open mods folder]                         (↻)
 ------------------------------------------------------------------------------
 (o) Amber  0.1.0                                                          (🗑)
     by Lilly · amber · folder · needs frost
     ⚠ Needs frost, which is not installed.                       (danger text)
 ------------------------------------------------------------------------------
 ( ) Wasm hello  0.1.0  [contains code]                    [Update to 0.2.0] (🗑)
     by Lilly · wasm_hello · folder
 ------------------------------------------------------------------------------
```

`(o)` is a switch; turning a mod on asks first (the declared-contents dialog). The tag `contains code` is the only
coloured text in a row (WARN); problems are DANGER text with a warning icon.

## Mods: browse

```
 [⌕ Search mods        ] [All tags v]  (o) Only fitting mods                    (↻)
 +--------------------------------+ | Amber
 | Amber           installed fits | | amber · Lilly · MIT
 | An amber block and its ore, …  | | [blocks] [example] [side: both]
 +--------------------------------+ |
   Red moon     installed fit unk.  | [Install 0.2.0]  4 KB · fits
   A blood red moon that throws …   | Downloads are checked against the index's SHA-256 …
                                    | description, links, screenshots
                                    | Versions
                                    | ----------------------------------------------
                                    | 0.2.0  2026-11-10  game >=0.2.0, <0.4.0  fits
```

The list is 320 wide, rows are 60 high; the selected row is RAISED. A hairline separates list and details. Below 720
wide the page shows the list or the details with a Back button.

## Game output

```
 Game output                                            [Copy] [Open log folder]
 Running for 4:05 (process 4242)                                          [Stop]
 /…/data/build/current/castanea --name Anna                       (mono, muted)
 +------------------------------------------------------------------------------+
 | castanea 0.2.0 starting in …                                     (BASE, mono) |
 | warning: …                                                         (WARN)     |
 +------------------------------------------------------------------------------+
```

After the end: `Ended normally after 4:05 (exit code 0)`, or `Crashed after 0:12 (exit code 101)` in DANGER with each
crash report and a Show report link. Lines with `error` or `panic` are DANGER, `warn` WARN; the view sticks to the
bottom.

## Settings

One column, at most 720 wide; a label column of 220 (label and a muted hint below it) and the control on the right;
a hairline under each row.

```
 Settings
 General
 ------------------------------------------------------------------
 Language                         [English              v]
 ------------------------------------------------------------------
 Offline mode                     ( )
 No update checks and no …
 ------------------------------------------------------------------
 Game
 ------------------------------------------------------------------
 Player name                      [Anna                  ]
 Relay server                     [https://              ]     backend_url
 Graphics backend                 [ Game setting | Vulkan | wgpu ]
 Window size                      [1280x720              ]
 Extra arguments                  [                      ]
 Environment                      [NAME     ] [value    ] (x)   [Add variable]
 Game folder  /…/data/game        [Open]
 Mod indexes
 ------------------------------------------------------------------
 /…/docs/examples/index.json                                 (🗑)
 [https://…/index.json         ] [Add] [Local file…]
 > Advanced                                       (closed by default)
   Local build folder             [Choose…] [Use releases]
   GitHub token                   [github_pat_…   ] [Save]
   Output lines kept              [5000]
   Settings / Data / Cache folder [Open]
```

Opening Advanced scrolls it into view.

## About

Name, version, one sentence, the repository link, the licence and font credits; Keyboard (rows of keys and what they
do); Safety (one paragraph). Same column as Settings.

## The brand font

`crates/ui/assets/fonts/castanea/` holds the project's own font, made for the launcher: `castanea.png` (a 2048 x 1024
RGBA atlas) and `castanea.fnt` (BMFont text format, line height 235, base 180). No foreign licence; see its
`SOURCE.md`. What it is:

- Riveted copper plate capitals with a baked bevel, highlight and drop shadow: a coloured display face, not a
  distance field and not plain coverage. It has A-Z (the lower-case ids 97-122 draw the same capitals), 0-9 and the
  space: 63 characters. No punctuation, no umlauts, no ß, no accents, no kerning pairs, no padding or spacing.
- Capitals are 171 of the 235 px line; the shadow runs 15 px below the baseline. Letters are wide and heavy, and
  rivets and plate seams are texture, not shape, so at small sizes only the letter shapes survive. Sizes below a 20 pt
  line height (15 px capitals) were not tried and are not advised; 24 to 30 read clearly in the screenshots.

So it is a brand face, not a text face, and the launcher uses it only where a few capital words are shown big:

| Where | Size token | Notes |
|---|---|---|
| wordmark on Play | `size::BRAND` 52 | `castanea` |
| page titles | `size::HEADING` 30 | Mods, Game output, Settings, About's `castanea launcher` and their German names |
| Play / Stop label | `size::BRAND_BUTTON` 28 | copper on a dark plate with an ACCENT outline; copper on the chestnut fill has no contrast |

Everything else stays Inter: body text, section titles, nav labels, the log (monospace), numbers such as the running
time. Text the font cannot spell (a title with ü, ß, a comma, a colon or a question mark) is set whole in Inter
semibold in the TEXT colour instead, never glyph by glyph, so no word is ever half copper. All current titles are
spelled by it in both languages (`tests/brand.rs` checks that), so a new title with punctuation shows up as an Inter
heading until the missing glyphs are added to the atlas.

How it is drawn (`bmfont.rs`, `bmfont/atlas.rs`, `bmfont/paint.rs`):

- `BitmapFont::parse` reads the BMFont text format (info, common, page, chars, kernings; one page) and lays out
  strings: advances, kerning, line wrapping at spaces, a baseline that puts the capitals' top at the box top
  (`cap_box`).
- egui has no bitmap fonts and a 235 px atlas drawn at 20 px would shimmer. Each glyph is box-filtered (exact area
  average on premultiplied pixels, so no colour fringes) into levels at 1/sqrt(2) steps, packed with a transparent
  border so no neighbour bleeds in. The painter picks the level just sharper than the screen size
  (`size * pixels_per_point / 235`), uploads it once as a texture with linear filtering and caches it in the context.
  The GPU only ever shrinks by less than 1.42x.
- Glyph corners snap to whole physical pixels at any `pixels_per_point`.
- The atlas is multiplied by the tint: white keeps the copper as drawn, other colours darken or colour it.
- `BmText::show` registers a heading with the real text for screen readers; the fallback keeps it too. An idle window
  still asks for no repaint (nothing animates).

## Components

| Component | Notes |
|---|---|
| primary button | ACCENT fill, white semibold text, radius 5, 32 high (Play: 240 x 48; its brand-font label sits on a dark plate with an ACCENT outline); hover and press darken in 120 ms. Once per view at most |
| secondary button | no fill, 1 px BORDER_STRONG outline, TEXT; HOVER and PRESSED fills |
| danger button | DANGER_FILL, white text; only in the remove question |
| quiet link | muted text, TEXT under the pointer; Game output, Try again, Show report |
| icon button | 32 square, a line icon, no fill until hovered; the tooltip is its name for screen readers |
| sidebar item | icon and label, 32 high, RAISED when selected |
| switch | 32 x 18 track: RAISED with a hairline when off, ACCENT when on; the knob slides in 120 ms |
| segmented control | a hairline outline, the selected part RAISED with TEXT, the others muted |
| text field | BASE fill, hairline outline, the accent outline when focused |
| tag | small muted text in a hairline outline |
| settings row | label column 220, control right, hairline below |
| dialog | RAISED, BORDER_STRONG hairline, radius 5, no shadow, a flat dimmed backdrop; Esc cancels, Enter confirms |
| toast | RAISED with a hairline, bottom right, no animation; 6 s (errors 10 s, DANGER text); a click dismisses it |
| focus ring | 1.5 px ACCENT, 2 px outside the control |

## Tokens

Colours (`theme::color`):

| Token | Value | Use |
|---|---|---|
| `BASE` | `#131211` | sidebar, log, fields |
| `BG` | `#191817` | page background |
| `RAISED` | `#22211f` | selected item, dialogs, toasts, switch track |
| `HOVER` / `PRESSED` | `#292725` / `#302e2b` | pointer feedback |
| `BORDER` | `#2e2c29` | hairlines |
| `BORDER_STRONG` | `#45423e` | outlines of buttons, dialogs, the off switch |
| `TEXT` | `#ebe7e3` | text |
| `TEXT_MUTED` | `#a39d97` | second lines, hints, status |
| `TEXT_DISABLED` | `#66615c` | disabled controls only |
| `ACCENT` | `#a3602f` | muted chestnut: Play, Install, switch on, focus, progress |
| `ACCENT_HOVER` / `ACCENT_PRESSED` | `#96582b` / `#885027` | the primary button under the pointer |
| `ON_ACCENT` | `#ffffff` | text on the accent and on DANGER_FILL |
| `DANGER` | `#e0786c` | error and crash text |
| `DANGER_FILL` | `#8f3a32` | the remove button in its question |
| `WARN` | `#d6ae5e` | `contains code`, warnings in the log |

All text colours meet WCAG AA (4.5:1) on BASE, BG, RAISED and HOVER, white meets it on the accent fills, and the
accent has 3:1 against the surfaces for focus rings; `theme::tests` checks this.

Spacing `space::{XS 4, S 8, M 12, L 16, XL 24, XXL 32}`; radius `radius::{M 5, S 4}` (and full for the switch); text
`size::{SMALL 12, BODY 14, TITLE 18}` in Inter regular and semibold (the log and the command line use the monospace
font at 12) and `size::{BRAND 52, HEADING 30, BRAND_BUTTON 28}` as line heights of the brand font; icons 16; lines `line::{HAIRLINE 1, FOCUS 1.5, PROGRESS 2}`; durations
`anim::{HOVER 0.12, TOAST 6 s, TOAST_ERROR 10 s}`; layout `layout::{SIDEBAR 184, CONTROL 32, PLAY 240x48, READABLE
720, LABEL 220, FIELD 320, LIST 320, NARROW 720, MIN 1000x640}`.

## Performance rules

- No repaint when idle: hover, press, switch and segment fades use `animate_bool_with_time`, which repaints only
  while moving; there are no other animations. A toast asks for one repaint when it expires; a running game or a job
  repaints at 4 Hz (the log, the progress, the running time).
- A test checks that an idle frame asks for no repaint, on every page.
