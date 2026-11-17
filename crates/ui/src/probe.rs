//! Named rectangles of the drawn layout, recorded only in tests, so they can check that regions do not overlap.

use eframe::egui;

#[cfg(test)]
#[derive(Clone, Default)]
pub struct Probe(pub Vec<(String, egui::Rect)>);

#[cfg(test)]
fn id() -> egui::Id {
    egui::Id::unique("layout-probe")
}

/// Records `rect` under `name` for this frame.
#[cfg(test)]
pub fn mark(ctx: &egui::Context, name: impl Into<String>, rect: egui::Rect) {
    let name = name.into();
    ctx.data_mut(|d| d.get_temp_mut_or_default::<Probe>(id()).0.push((name, rect)));
}

#[cfg(not(test))]
#[inline]
pub fn mark(_ctx: &egui::Context, _name: impl Into<String>, _rect: egui::Rect) {}

/// Clears the record at the start of a frame.
#[cfg(test)]
pub fn clear(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(id(), Probe::default()));
}

#[cfg(not(test))]
#[inline]
pub fn clear(_ctx: &egui::Context) {}

/// What the last frame recorded.
#[cfg(test)]
pub fn take(ctx: &egui::Context) -> Vec<(String, egui::Rect)> {
    ctx.data(|d| d.get_temp::<Probe>(id())).map(|p| p.0).unwrap_or_default()
}
