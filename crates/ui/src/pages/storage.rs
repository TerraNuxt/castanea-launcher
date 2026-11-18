//! Settings → Storage and reset: one row per kind of data the launcher keeps (name, real path, size, its own Delete),
//! then 'Delete everything' and, on Windows, 'Uninstall launcher'. The questions are built here and drawn by `shell`.

use castanea_launcher_core::storage::{self, Row};
use eframe::egui;

use crate::app::{App, Confirm};
use crate::storage::{Deletion, bytes_text, row_hint_key, row_name_key, shown_path};
use crate::theme::{color, layout, space};
use crate::widgets::{self, Button, Kind};

/// Width of the right side of a row: the size, then the Delete button.
const RIGHT: f32 = 196.0;
const SIZE_SLOT: f32 = 84.0;

pub fn ui(app: &mut App, ui: &mut egui::Ui) {
    if app.reveal_storage == Some(egui::Align::Min) {
        app.reveal_storage = None;
        ui.scroll_to_cursor(Some(egui::Align::Min));
    }
    if !app.storage.measuring {
        app.measure_storage(ui.ctx());
    }
    widgets::section(ui, app.tr.get("storage.section"));
    ui.add_space(space::S);
    widgets::wrapped(ui, widgets::small(app.tr.get("storage.intro")));
    if let Some(why) = app.storage_blocked() {
        ui.add_space(space::XS);
        widgets::wrapped(ui, egui::RichText::new(app.tr.get(why)).size(crate::theme::size::SMALL).color(color::WARN));
    }
    for row in Row::ALL {
        if row != Row::GameData || storage::knows_game_data(&app.paths) {
            row_ui(app, ui, row);
        }
    }
    everything_row(app, ui);
    #[cfg(windows)]
    uninstall_row(app, ui);
    if app.reveal_storage == Some(egui::Align::Max) {
        app.reveal_storage = None;
        ui.scroll_to_cursor(Some(egui::Align::Max));
    }
}

/// One row: name, hint and path on the left; the size (right-aligned) and the button on the right.
fn frame_row(ui: &mut egui::Ui, left: impl FnOnce(&mut egui::Ui), right: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(space::M);
    let width = ui.available_width();
    ui.horizontal(|ui| {
        let side = egui::Layout::top_down(egui::Align::Min);
        let left_width = width - RIGHT - space::L;
        ui.allocate_ui_with_layout(egui::vec2(left_width, layout::CONTROL), side, |ui| {
            // A fixed width, so the right side lines up in every row whatever the text.
            ui.set_min_width(left_width);
            ui.set_max_width(left_width);
            ui.spacing_mut().item_spacing.y = layout::LINE_GAP;
            left(ui);
        });
        ui.allocate_ui_with_layout(
            egui::vec2(RIGHT, layout::CONTROL),
            egui::Layout::right_to_left(egui::Align::Center),
            right,
        );
    });
    ui.add_space(space::M);
    widgets::hairline(ui);
}

fn row_ui(app: &mut App, ui: &mut egui::Ui, row: Row) {
    let info = app.storage.info.get(&row).copied();
    let name = app.tr.get(row_name_key(row)).to_owned();
    let hint = app.tr.get(row_hint_key(row)).to_owned();
    let path = shown_path(&app.paths, row).display().to_string();
    let size =
        info.map_or_else(|| "…".to_owned(), |i| if i.present { bytes_text(i.size.bytes) } else { "–".to_owned() });
    let can = app.storage_blocked().is_none() && info.is_some_and(|i| i.present);
    let failures = app.storage.failures.get(&row).cloned().unwrap_or_default();
    let delete = app.tr.get("storage.delete").to_owned();
    let button_name = app.tr.fill("storage.delete_name", &[("name", &name)]);
    let (mut ask, mut retry) = (false, false);
    frame_row(
        ui,
        |ui| {
            ui.label(widgets::text(&name));
            widgets::wrapped(ui, widgets::small(&hint));
            widgets::wrapped(ui, widgets::small(&path));
            for failure in &failures {
                let line = app.tr.fill(
                    "storage.failed",
                    &[("path", &failure.path.display().to_string()), ("error", &failure.error)],
                );
                widgets::wrapped(ui, egui::RichText::new(line).size(crate::theme::size::SMALL).color(color::DANGER));
            }
            if !failures.is_empty() {
                ui.add_space(space::XS);
                let name = app.tr.fill("storage.retry_name", &[("name", &name)]);
                let enabled = app.storage_blocked().is_none();
                retry = Button::new(Kind::Secondary, app.tr.get("storage.retry"))
                    .enabled(enabled)
                    .name(name)
                    .show(ui)
                    .clicked();
            }
        },
        |ui| {
            ask = Button::new(Kind::Destructive, &delete).enabled(can).name(button_name).show(ui).clicked();
            ui.allocate_ui_with_layout(
                egui::vec2(SIZE_SLOT, layout::CONTROL),
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| ui.label(widgets::muted(&size)),
            );
        },
    );
    if ask {
        app.storage.typed.clear();
        app.confirm = Some(if row == Row::GameData { Confirm::DeleteGameData } else { Confirm::DeleteRow(row) });
    }
    if retry {
        app.start_delete(ui.ctx(), Deletion::Row(row));
    }
}

fn everything_row(app: &mut App, ui: &mut egui::Ui) {
    let can = app.storage_blocked().is_none();
    let label = app.tr.get("storage.all").to_owned();
    let hint = app.tr.get("storage.all_hint").to_owned();
    let mut ask = false;
    frame_row(
        ui,
        |ui| {
            ui.label(widgets::strong(&label, crate::theme::size::BODY));
            widgets::wrapped(ui, widgets::small(&hint));
        },
        |ui| ask = Button::new(Kind::Destructive, &label).enabled(can).show(ui).clicked(),
    );
    if ask {
        app.confirm = Some(Confirm::DeleteEverything);
    }
}

#[cfg(windows)]
fn uninstall_row(app: &mut App, ui: &mut egui::Ui) {
    let can = app.storage_blocked().is_none() && app.launcher_layout.is_some();
    let label = app.tr.get("storage.uninstall").to_owned();
    let hint = app.tr.get("storage.uninstall_hint").to_owned();
    let button = app.tr.get("storage.uninstall_button").to_owned();
    let mut ask = false;
    frame_row(
        ui,
        |ui| {
            ui.label(widgets::strong(&label, crate::theme::size::BODY));
            widgets::wrapped(ui, widgets::small(&hint));
        },
        |ui| ask = Button::new(Kind::Destructive, &button).enabled(can).name(label.clone()).show(ui).clicked(),
    );
    if ask {
        app.confirm = Some(Confirm::Uninstall);
    }
}

/// What a storage question says and asks for.
pub struct Dialog {
    pub title: String,
    pub intro: String,
    /// Groups of lines: a heading and the paths below it.
    pub groups: Vec<(String, Vec<String>)>,
    pub outro: String,
    pub yes: String,
    /// The word to type before the button works.
    pub type_word: bool,
    /// Only a Close button.
    pub notice: bool,
}

fn paths_of(row: Row, app: &App) -> Vec<String> {
    storage::targets(&app.paths, row).iter().map(|p| p.display().to_string()).collect()
}

/// The question for a storage confirm, or `None` for the other confirms.
pub fn dialog(app: &App, confirm: &Confirm) -> Option<Dialog> {
    let tr = &app.tr;
    let game = shown_path(&app.paths, Row::GameData).display().to_string();
    let own_groups = || -> Vec<(String, Vec<String>)> {
        Row::OWN.iter().map(|row| (tr.get(row_name_key(*row)).to_owned(), paths_of(*row, app))).collect()
    };
    let base = Dialog {
        title: String::new(),
        intro: String::new(),
        groups: Vec::new(),
        outro: String::new(),
        yes: tr.get("storage.delete").to_owned(),
        type_word: false,
        notice: false,
    };
    Some(match confirm {
        Confirm::DeleteRow(row) => {
            let name = tr.get(row_name_key(*row)).to_owned();
            Dialog {
                title: tr.fill("storage.row_title", &[("name", &name)]),
                intro: tr.get("storage.row_body").to_owned(),
                groups: vec![(name, paths_of(*row, app))],
                ..base
            }
        }
        Confirm::DeleteGameData => Dialog {
            title: tr.get("storage.game_data_title").to_owned(),
            intro: tr.fill("storage.game_data_body", &[("path", &game)]),
            groups: vec![(tr.get("storage.game_data").to_owned(), paths_of(Row::GameData, app))],
            type_word: true,
            ..base
        },
        Confirm::DeleteEverything => Dialog {
            title: tr.get("storage.all_title").to_owned(),
            intro: tr.get("storage.all_body").to_owned(),
            groups: own_groups(),
            outro: tr.fill("storage.keeps", &[("path", &game)]),
            yes: tr.get("storage.all").to_owned(),
            ..base
        },
        #[cfg(windows)]
        Confirm::Uninstall => uninstall_dialog(app, base, own_groups(), &game),
        Confirm::Goodbye => Dialog {
            title: tr.get("storage.done_title").to_owned(),
            intro: tr.fill("storage.done_body", &[("path", &game)]),
            yes: tr.get("common.close").to_owned(),
            notice: true,
            ..base
        },
        _ => return None,
    })
}

/// The uninstall question: the data, the shortcuts, the program, and what happens to its folder.
#[cfg(windows)]
fn uninstall_dialog(app: &App, base: Dialog, mut groups: Vec<(String, Vec<String>)>, game: &str) -> Dialog {
    let tr = &app.tr;
    let mut outro = tr.fill("storage.keeps", &[("path", game)]);
    if let Some(layout) = &app.launcher_layout {
        let list =
            storage::uninstall_plan(&app.paths, &layout.exe, &storage::shared_folders(), &storage::shortcut_folders());
        if !list.shortcuts.is_empty() {
            let lines = list.shortcuts.iter().map(|p| p.display().to_string()).collect();
            groups.push((tr.get("storage.shortcuts").to_owned(), lines));
        }
        let program = std::iter::once(&list.exe).chain(&list.leftovers).chain(&list.shipped);
        groups.push((tr.get("storage.program").to_owned(), program.map(|p| p.display().to_string()).collect()));
        let note = match &list.dir {
            storage::InstallDir::Owned(dir) => tr.fill("storage.dir_owned", &[("path", &dir.display().to_string())]),
            storage::InstallDir::Shared(dir) => tr.fill("storage.dir_shared", &[("path", &dir.display().to_string())]),
        };
        outro = format!("{outro} {note}");
    }
    Dialog {
        title: tr.get("storage.uninstall_title").to_owned(),
        intro: tr.get("storage.uninstall_body").to_owned(),
        groups,
        outro,
        yes: tr.get("storage.uninstall_button").to_owned(),
        ..base
    }
}

/// Draws the groups and the typed field; returns whether the button may be pressed.
pub fn dialog_body(typed: &mut String, tr: &crate::i18n::Strings, ui: &mut egui::Ui, dialog: &Dialog) -> bool {
    if !dialog.groups.is_empty() {
        egui::ScrollArea::vertical().max_height(layout::DIALOG_LIST).auto_shrink([true, true]).show(ui, |ui| {
            for (heading, lines) in &dialog.groups {
                ui.add_space(space::S);
                ui.label(widgets::strong(heading, crate::theme::size::BODY));
                for line in lines {
                    widgets::wrapped(ui, widgets::small(line));
                }
            }
        });
    }
    if !dialog.outro.is_empty() {
        ui.add_space(space::M);
        widgets::wrapped(ui, widgets::muted(&dialog.outro));
    }
    if !dialog.type_word {
        return true;
    }
    ui.add_space(space::L);
    widgets::wrapped(ui, widgets::small(tr.fill("storage.type", &[("word", "DELETE")])));
    ui.add_space(space::XS);
    let field = widgets::text_field(ui, typed, "DELETE", layout::FIELD);
    if ui.memory(|m| m.focused().is_none()) {
        field.request_focus();
    }
    storage::is_confirmation(typed)
}
