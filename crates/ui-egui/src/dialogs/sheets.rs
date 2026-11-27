use egui::{ComboBox, Ui};
use gridcraft_engine::model::{Visibility, Workbook};

use super::show::ok_cancel_enabled;
use super::{Dialog, Json, SheetApp, json};

pub(super) fn move_sheet(app: &SheetApp, ui: &mut Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let Some(wb) = app.session.active().map(|doc| doc.wb.as_ref()) else {
        *open = false;
        return;
    };
    let Some(source) = source_sheet(wb, d.values.get("sheet")) else {
        *open = false;
        return;
    };
    let lang = app.ui.language;
    let end = wb.sheets.len();
    let was_copy = d.values.get("copy").and_then(Json::as_bool).unwrap_or(false);
    let to = d.values.get("to").and_then(Json::as_u64).and_then(|n| usize::try_from(n).ok()).unwrap_or(0);
    // `sheet.move` inserts at `to` after removing the source for a move, but keeps the source
    // in place for a copy. Keep the named destination when the user toggles Create a copy.
    let before = if was_copy || to < source { to } else { to.saturating_add(1) }.min(end);
    let mut copy = was_copy;
    let options = wb
        .sheets
        .iter()
        .enumerate()
        .filter(|(index, _)| was_copy || *index != source)
        .map(|(index, sheet)| (index, sheet.name.as_str()))
        .chain([(end, lang.tr("(move to end)"))]);
    let (picked, menu_open) = sheet_picker(ui, lang.tr("Before sheet:"), Some(before), options);
    ui.checkbox(&mut copy, lang.tr("Create a copy"));
    let before = picked.unwrap_or(before);
    if picked.is_some() || copy != was_copy {
        let to = if copy || before <= source { before } else { before - 1 };
        d.set_value("to", json!(to));
        d.set_value("copy", json!(copy));
    }
    ok_cancel_enabled(ui, confirm, open, !menu_open && !wb.protected_structure);
}

pub(super) fn unhide_sheet(app: &SheetApp, ui: &mut Ui, d: &mut Dialog, confirm: &mut bool, open: &mut bool) {
    let Some(wb) = app.session.active().map(|doc| doc.wb.as_ref()) else {
        *open = false;
        return;
    };
    let lang = app.ui.language;
    let options = wb.sheets.iter().filter(|sheet| sheet.visibility != Visibility::Visible).map(|sheet| (sheet.name.as_str(), sheet.name.as_str()));
    let selected = d.values.get("sheet").and_then(Json::as_str).filter(|name| options.clone().any(|(option, _)| option == *name));
    let (picked, menu_open) = sheet_picker(ui, lang.tr("Unhide sheet:"), selected, options);
    let chosen = picked.or(selected);
    let can_unhide = chosen.is_some() && !wb.protected_structure && !menu_open;
    if let Some(name) = picked {
        d.set_value("sheet", json!(name));
    }
    ok_cancel_enabled(ui, confirm, open, can_unhide);
}

fn source_sheet(wb: &Workbook, value: Option<&Json>) -> Option<usize> {
    match value {
        Some(Json::String(name)) => wb.sheet_index(name),
        Some(idx) if !idx.is_null() => idx.as_u64().and_then(|index| usize::try_from(index).ok()).filter(|index| *index < wb.sheets.len()),
        _ => Some(wb.active_sheet()),
    }
}

fn sheet_picker<'a, T: Copy + PartialEq>(
    ui: &mut Ui,
    label: &str,
    selected: Option<T>,
    options: impl Iterator<Item = (T, &'a str)> + Clone,
) -> (Option<T>, bool) {
    let label = ui.label(label);
    let choice = options.clone().find(|(value, _)| Some(*value) == selected).map(|(_, text)| text).unwrap_or_default();
    let mut picked = None;
    let response = ComboBox::from_id_salt("sheet_choice")
        .selected_text(choice)
        .width(320.0)
        .height(220.0)
        .show_ui(ui, |ui| {
            for (value, name) in options {
                if ui.selectable_label(Some(value) == selected, name).clicked() {
                    picked = Some(value);
                }
            }
        })
        .response
        .labelled_by(label.id);
    (picked, ComboBox::is_open(ui.ctx(), response.id))
}
