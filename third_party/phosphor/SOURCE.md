# Phosphor icons

- Files: `crates/ui/assets/fonts/phosphor-regular.ttf`
- From: the Phosphor icon font (regular weight, line icons) as shipped in the `egui-phosphor` 0.14.0 crate
  (`res/Phosphor.ttf`), https://phosphoricons.com, https://github.com/phosphor-icons/core
- Licence: MIT (`LICENSE`, Copyright (c) 2023 Phosphor Icons)
- Changes: subset with `pyftsubset` (fonttools) to a list of code points, no layout features, no hinting. The subset
  was made for an earlier, longer icon list and still holds a few glyphs `crates/ui/src/icons.rs` no longer names. To
  add an icon: add its code point to `icons.rs` and to the subset list, and subset again from the full font.
