# Inter

- Files: `crates/ui/assets/fonts/inter-regular.ttf`, `inter-semibold.ttf`
- From: Inter 4.1, `extras/ttf/Inter-{Regular,SemiBold}.ttf` of
  https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip
- Licence: SIL Open Font License 1.1 (`LICENSE.txt`, Copyright (c) 2016 The Inter Project Authors)
- Changes: subset with `pyftsubset` (fonttools) to Basic Latin, Latin-1 and common punctuation (U+0020-007E,
  U+00A0-00FF, dashes, quotes, bullet, ellipsis, arrows, euro), kerning kept, no hinting. The subset keeps the
  font names; under the OFL a modified font may keep the name "Inter" because Inter declares no Reserved Font Name.
