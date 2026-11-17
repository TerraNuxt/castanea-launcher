# castanea launcher: UI design

The launcher's look: a slim icon sidebar on the left, a wide content area, a dark warm theme with a chestnut orange
accent, rounded cards, generous spacing. Every colour, size and duration is a token in `crates/ui/src/theme.rs`; pages
build on the components in `crates/ui/src/widgets/`. All pictures are drawn in code (the hero, the logo mark, the
profile tiles); icons come from the Phosphor icon font (MIT), text from Inter (OFL), see `third_party/`.

## Frame

```
+----+--------------------------------------------------------------------------+
| ## | (logo) castanea launcher                     [progress] [offline]  _ [] X|  title bar (drag, double click maximises)
+----+--------------------------------------------------------------------------+
| H  |                                                                          |
| M  |                         page content                                     |
| P  |                                                                          |
| V  |                                                                          |
|    |                                                                          |
| N• |                                                                          |  N = notifications, • = unread badge
| S  |                                                                          |
| i  |                                                                          |
|[>] |                                                                          |  green Play, pinned at the bottom
+----+--------------------------------------------------------------------------+
```

Sidebar items (top): Home, Mods, Profiles, Versions. Bottom: Notifications, Settings, About, Play. Each item is an
icon with a tooltip and an AccessKit label; the active one has an accent pill and the filled icon. Ctrl+1 … Ctrl+7 open
the pages in sidebar order, F5 plays, Ctrl+F searches mods, Esc closes a dialog or goes back.

With "Use the system window frame" off (the default) the launcher draws its own title bar with minimise, maximise and
close, and resize handles on the edges; on, the system frame is used and the title bar keeps only the status chips.

## Home

```
  (mark) Welcome back, Anna!                                               
+-------------------------------------------------------+  +---------------------+
|  hero: sky gradient, hills, a chestnut, the profile    |  | NEWS          (org) |
|        Survival                                        |  | title · date        |
|        castanea 0.2.0 · world "lake"                   |  | body…               |
|              [ (S) Survival          v ]          [✎]  |  +---------------------+
+-------------------------------------------------------+  | MODS          (vio) |
 (clock) Last played profiles                    3 profiles|  | 2 updates / featured|
+-------------------------------------------------------+  | [Update all]        |
| [S] Survival                       [▶ Play]  [⚙]       |  +---------------------+
|     castanea 0.2.0 · world · 2 h ago                   |  | MULTIPLAYER   (blu) |
+-------------------------------------------------------+  | [address] [Join]    |
| [C] Creative test   (running, green outline)            |  | Open my world  (o)  |
|     castanea 0.2.0 · menu · now  [Output] [■ Stop] [⚙] |  | relay: none         |
+-------------------------------------------------------+  +---------------------+
 [ ⊞ Manage all profiles (orange) ] [ + Add mods (dark) ]   | SHORTCUTS     (grn) |
                                                            +---------------------+
```

The right column is 320 wide when the content is at least 960 wide; below that it collapses into a two column grid
under the profile list (in the same scroll area). The hero is 34 % of the height (150 to 260). A card for a running game
gets a green outline, Output (the log page) and a red Stop. Profiles never run twice at once (they share a folder), but
different profiles run side by side, each with its own log.

## Mods: installed

```
 Mods                                   [ Installed | Browse ]   for profile Survival
 [Import zip…] [Import folder…] [Open mods folder] [↻]
+--------------------------------------------------------------------------------+
| (o) Amber 0.1.0 by Lilly          code · client        amber · folder  [↑][🗑]  |
|     Needs: frost                                                               |
|     ⚠ Needs frost, which is not installed.                                      |
+--------------------------------------------------------------------------------+
```

`(o)` is a switch; turning a mod on asks first (the declared-contents dialog).

## Mods: browse and details

```
 [🔍 Search mods      ] [All tags v] (o) Only fitting  [↻]
+-----------------------------+  +---------------------------------------------+
| Amber          fits      ✓  |  | [icon] Amber                                 |
| An amber block and its ore… |  |        amber · Lilly · MIT                   |
+-----------------------------+  | #blocks #example  side: both                 |
| Red moon      fit unknown   |  | +-----------------------------------------+ |
| A blood red moon…           |  | | [Install 0.2.0]  4 KB  fits              | |
+-----------------------------+  | | Downloads are checked against SHA-256…   | |
                                 | +-----------------------------------------+ |
                                 | description, links, screenshots, versions    |
                                 +---------------------------------------------+
```

Below 760 wide the list and the details share one column: the details get a Back button.

## Profiles

```
 Profiles                                                     [+ New profile]
+---------------------+  +----------------------------------------------------+
| [S] Survival      ● |  | General:  Name [        ]  Player [       ]        |
| [C] Creative test   |  | Start in: [ Menu | A world | Join a server ]       |
|                     |  |           world, seed, host port / address        |
|                     |  | Game:     version [Newest (0.2.0) v]               |
|                     |  | Graphics: [ Game setting | Vulkan | wgpu ] window  |
|                     |  | Advanced: relay, extra arguments, environment     |
|                     |  | [Open game folder]               [Remove profile]  |
+---------------------+  +----------------------------------------------------+
```

## Versions

```
 Game versions                                  [Add folder…] [Search again]
+--------------------------------------------------------------------------------+
| (cube) 0.2.0   local folder · checked              [Open] [Remove]             |
|        /path/to/build                                                          |
+--------------------------------------------------------------------------------+
 Releases                                                              [↻]
+--------------------------------------------------------------------------------+
| castanea 0.3.0   pre-release · 2026-11-10                 [⬇ Download (12 MB)]  |
| ▸ Release notes                                                                |
+--------------------------------------------------------------------------------+
```

## Settings

```
 Settings
+-- General ------------------------------+  +-- Game sources ------------------+
| Language        [System v]              |  | GitHub TerraNuxt/castanea  [🗑]   |
| Offline mode    (o)                     |  | [owner/repository] [Add]        |
| Window frame    (o) system frame        |  | token: …  [Save token]          |
| Log lines       [5000]                  |  +---------------------------------+
+-----------------------------------------+  +-- Mod indexes -------------------+
+-- Folders ------------------------------+  | …                                |
| Settings  path  [Open]                  |  +---------------------------------+
+-----------------------------------------+
```

Two columns when the content is at least 960 wide, else one.

## About

```
 [mark] castanea launcher  0.1.0
 text, link, licence line (Inter OFL, Phosphor MIT)
 +-- Keyboard ----------+  +-- Safety -------------------+
 | Ctrl+1…7  pages      |  | HTTPS only, SHA-256, …      |
 +----------------------+  +-----------------------------+
```

## Game output (log)

```
 Game output     [Survival ●] [Creative test]           [Copy] [Open log folder]
 +-- status: running (process 4242) / ended normally / crashed, with reports ---+
 | castanea 0.2.0 starting in …                                                 |
 | warning: …                                                                   |
 +------------------------------------------------------------------------------+
```

Lines with `error` or `panic` are red; the view sticks to the bottom.

## Notifications

```
 Notifications                                                   [Clear all]
 +-- (i) Installed amber. It is disabled until you enable it.    12:04  [x] --+
 +-- (!) index.json: not found                                    12:01  [x] --+
```

New notes also show for 5 seconds as a toast at the bottom right (slides in, fades out); the bell shows the unread count.

## Components

| Component | Where | Notes |
|---|---|---|
| card | everywhere | `surface` fill, `radius.l`, optional accent stripe on top (right column) or outline (running) |
| pill button | Play, Stop, Install, actions | variants: play (green), danger (red), accent (orange), ghost (dark); icon + text; hover lightens, press darkens and shrinks by 1 px |
| icon button | gear, edit, remove, window controls | square, `radius.m`, tooltip = AccessKit label |
| sidebar item | sidebar | 44 px square, accent pill on the left when active, filled icon, badge |
| hero | home | procedural sky, sun, three hill layers, a chestnut; colours from the profile id |
| list row | mods, versions, releases, browse | card with a title line, a muted line and actions on the right |
| switch | settings, mods | animated knob, AccessKit check box |
| segmented control | mods views, profile options | one pill per choice |
| chip | tags, status | small rounded label |
| toast / notification | bottom right / notifications page | colour by kind, slide and fade |
| dialog | confirmations | dimmed backdrop, card in the middle, Esc cancels, Enter confirms |
| text field | search, settings | sunken fill, focus ring in the accent |

## Tokens

Colours (`theme::Palette`):

| Token | Value | Use |
|---|---|---|
| `bg` | `#131110` | content background |
| `sidebar` | `#0c0b0a` | sidebar and title bar |
| `surface` | `#1d1a18` | cards |
| `surface_hover` | `#262320` | hovered cards and ghost buttons |
| `sunken` | `#151312` | text fields, log |
| `border` | `#2f2a26` | card outline |
| `text` | `#f3ede7` | text |
| `text_muted` | `#a69c93` | second lines |
| `text_faint` | `#6e655e` | hints |
| `accent` | `#e07a2f` | chestnut orange: active item, links, focus |
| `accent_strong` | `#b85a1c` | gradients, pressed |
| `play` | `#2faa58` | Play, running |
| `danger` | `#d6453b` | Stop, remove, errors |
| `warn` | `#e5ad48` | warnings |
| `info` | `#4b8fe0` | multiplayer card, info notes |
| `violet` | `#8a6ad8` | mods card |
| `teal` | `#2aa596` | shortcuts card |

Spacing scale `space::{XS 4, S 8, M 12, L 16, XL 24, XXL 32}`; radii `radius::{S 6, M 10, L 14, PILL 255}`; text sizes
`size::{CAPTION 12, SMALL 13, BODY 14.5, BUTTON 15, TITLE 17, HEADING 22, DISPLAY 28}`; durations `anim::{HOVER 0.12,
PRESS 0.08, PAGE 0.16, TOAST 5 s}`; layout `layout::{SIDEBAR 72, TITLE_BAR 38, RIGHT_COLUMN 320, WIDE 960, MIN 1000x640}`.

## Performance rules

- No continuous repaint when idle: animations use `animate_bool_with_time`, which repaints only while moving; toasts ask
  for one repaint when they expire; a running game or a job repaints at 4 Hz.
- The hero is a few hundred vertices built per frame only when a frame is drawn.
- A test checks that an idle frame asks for no repaint.
