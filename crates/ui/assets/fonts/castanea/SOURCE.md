# Castanea Copper (the brand font)

- Files: `castanea.png` (glyph atlas, 2048 x 1024, RGBA, transparent background) and `castanea.fnt` (BMFont text
  format, line height 235, base 180).
- Origin: the project's own work, made for the launcher. No foreign file, no foreign licence; it is covered by the
  launcher's MIT licence like the rest of the repository.
- Content: riveted copper plate capitals A-Z (the lower-case ids 97-122 point at the same shapes), digits 0-9 and the
  space. No punctuation, no umlauts, no ß, no kerning. The atlas is coloured (copper with a baked bevel and shadow),
  not a distance field.
- Use: `crates/ui/src/bmfont.rs` (loader and layout) and `bmfont/` (scaled levels, painting). Only the wordmark, page
  titles and the Play/Stop label use it; text it cannot spell is set in Inter. See docs/ui-design.md, "The brand font".
