# Phosphor icons

- Files: `crates/ui/assets/fonts/phosphor-regular.ttf`, `crates/ui/assets/fonts/phosphor-fill.ttf`
- From: the Phosphor icon font (regular and fill weights) as shipped in the `egui-phosphor` 0.14.0 crate
  (`res/Phosphor.ttf`, `res/Phosphor-Fill.ttf`), https://phosphoricons.com, https://github.com/phosphor-icons/core
- Licence: MIT (`LICENSE`, Copyright (c) 2023 Phosphor Icons)
- Changes: subset with `pyftsubset` (fonttools) to the code points in `crates/ui/src/icons.rs`, no layout features,
  no hinting. To add an icon: add its code point to `icons.rs` and to the subset list, and subset again from the full font.
