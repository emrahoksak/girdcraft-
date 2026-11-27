use super::editor::popup_dismissed;
use super::geo::Geo;
use super::{Drag, FillTag, RangeRef, Rect, Sense, Sheet, SheetApp, Stroke, Tokens, col_to_letters, json, pos2, vec2};

/// Opens a read-only checklist for the clicked header. Criteria change only on Apply or Clear.
pub(super) fn filter_menu(app: &mut SheetApp, ui: &mut egui::Ui, geo: &Geo, was_open: bool) {
    let Some((col, row, at)) = app.grid.filter_menu else { return };
    let Some(d) = app.session.active() else { return };
    let wb = d.wb.clone();
    let si = wb.active_sheet();
    let Some(sh) = wb.sheet(si) else { return };
    let at_cell = gridcraft_engine::core::CellRef::new(row, col);
    let table = sh.table_at(at_cell).filter(|t| t.header_row && t.range.start.row == row);
    let name = table.map(|t| t.name.as_str());
    let sort_range = table
        .map(|t| t.data_range().map_or(t.range, |body| RangeRef::new(t.range.start, body.end)))
        .or_else(|| sh.autofilter.as_ref().map(|af| af.range));
    let Some(sort_range) = sort_range else {
        app.grid.filter_menu = None;
        return;
    };
    let _ = geo;
    let field_id = egui::Id::unique(("filter_menu", row, col));
    let lang = crate::i18n::current(ui.ctx());
    let column = col_to_letters(col);
    let params = |mut p: serde_json::Value| {
        if let Some(name) = name {
            p["table"] = json!(name);
        }
        p
    };
    // Enter applies from the search or a checkbox; a focused action button handles its own Enter.
    let on_button = ui.ctx().memory(|m| m.focused()).is_some_and(|f| app.grid.filter_buttons.contains(&f));
    let enter = !on_button && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
    let mut close = false;
    if !was_open {
        app.grid.filter_search.clear();
        app.grid.filter_matches = None;
    }
    let mut filter_matches = app.grid.filter_matches.take();
    let mut checks =
        app.grid.filter_checks.take().filter(|_| was_open).unwrap_or_else(|| gridcraft_engine::cmd::data::filter_choices_in(&wb, si, col, name));
    let area = egui::Area::new(field_id).fixed_pos(at + vec2(-180.0, 10.0)).order(egui::Order::Foreground);
    let response = area.show(ui.ctx(), |ui| {
        crate::widgets::context_frame(ui).show(ui, |ui| {
            use crate::icons::Icon;
            use crate::widgets::context_item;
            ui.set_width(244.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            let ascending = context_item(ui, Icon::SortAsc, lang.tr("Sort A to Z"));
            let descending = context_item(ui, Icon::SortDesc, lang.tr("Sort Z to A"));
            for (response, cmd) in [(&ascending, "data.sortAscending"), (&descending, "data.sortDescending")] {
                if response.clicked() {
                    app.run_or_alert(cmd, json!({"column": column, "range": sort_range.a1(), "header": true}));
                    close = true;
                }
            }
            ui.separator();
            ui.label(egui::RichText::new(lang.tr("Filter")).strong());
            let field = ui.add(egui::TextEdit::singleline(&mut app.grid.filter_search).hint_text(lang.tr("Search")).desired_width(f32::INFINITY));
            if !was_open {
                field.request_focus();
            }
            if field.changed() {
                let needle = app.grid.filter_search.trim().to_lowercase();
                filter_matches = (!needle.is_empty()).then(|| {
                    checks
                        .iter()
                        .enumerate()
                        .filter_map(|(i, c)| filter_choice_label(lang, c.value.is_none(), &c.label).to_lowercase().contains(&needle).then_some(i))
                        .collect()
                });
            }
            let matching = filter_matches.as_deref();
            let count = matching.map_or(checks.len(), <[usize]>::len);
            let index = |i| matching.map_or(i, |entries| entries[i]);
            let checked = (0..count).filter(|&i| checks[index(i)].checked).count();
            let mut all = checked == count;
            let label = if matching.is_none() { "(Select All)" } else { "(Select All Search Results)" };
            let mut any = checked > 0;
            if ui.checkbox(&mut all, lang.tr(label)).changed() {
                for i in 0..count {
                    checks[index(i)].checked = all;
                }
                any = all && count > 0;
            }
            let row_height = 22.0;
            let mut changed = false;
            let mut list = egui::ScrollArea::vertical().max_height(220.0).auto_shrink([false, true]);
            if field.changed() {
                list = list.vertical_scroll_offset(0.0);
            }
            list.show_rows(ui, row_height, count, |ui, visible| {
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                let size = vec2(ui.available_width(), row_height);
                for i in visible {
                    let c = &mut checks[index(i)];
                    let label = filter_choice_label(lang, c.value.is_none(), &c.label);
                    changed |= ui.add_sized(size, egui::Checkbox::new(&mut c.checked, label)).changed();
                }
            });
            if changed {
                any = (0..count).any(|i| checks[index(i)].checked);
            }
            ui.separator();
            ui.horizontal(|ui| {
                let clear = ui.button(lang.tr("Clear Filter"));
                if clear.clicked() {
                    app.run_or_alert("data.filterBy", params(json!({"column": column, "clear": true})));
                    close = true;
                }
                let apply = ui.add_enabled(any, egui::Button::new(lang.tr("Apply")));
                app.grid.filter_buttons = vec![ascending.id, descending.id, clear.id, apply.id];
                if apply.clicked() || (enter && any) {
                    // Preserve choices excluded by another column, but apply only the current
                    // search matches amongst the remaining choices.
                    if let Some(matching) = matching {
                        let mut matches = matching.iter().copied().peekable();
                        for (i, c) in checks.iter_mut().enumerate() {
                            c.checked &= matches.next_if_eq(&i).is_some();
                        }
                    }
                    let selection = gridcraft_engine::cmd::data::filter_choices_selection_in(&wb, si, col, name, &checks);
                    let selected: Vec<_> = selection.iter().map(|c| (c, c.checked)).collect();
                    let p = gridcraft_engine::cmd::data::filter_choices_params(&column, &selected);
                    app.run_or_alert("data.filterBy", params(p));
                    close = true;
                }
            });
        });
    });
    if close || popup_dismissed(&response.response, was_open) {
        app.grid.filter_search.clear();
        app.grid.filter_matches = None;
        app.grid.filter_buttons.clear();
        app.grid.filter_menu = None;
    } else {
        app.grid.filter_checks = Some(checks);
        app.grid.filter_matches = filter_matches;
    }
}

fn filter_choice_label(lang: crate::i18n::Language, blank: bool, label: &str) -> &str {
    if blank { lang.tr("(Blanks)") } else { label }
}

/// Shows the cell actions at the pointer or at the keyboard-selected cell.
pub(super) fn context_menu(app: &mut SheetApp, ui: &mut egui::Ui) {
    let Some(at) = app.grid.context_menu else { return };
    let mut open = true;
    let lang = crate::i18n::current(ui.ctx());
    let tokens = Tokens::get(ui.ctx());
    let style = move |style: &mut egui::Style| {
        style.spacing.item_spacing.y = 1.0;
        style.visuals.window_fill = tokens.menu_bg;
        style.visuals.window_stroke = Stroke::new(1.0, tokens.ribbon_border);
        style.visuals.window_corner_radius = 8.into();
        style.visuals.popup_shadow = egui::Shadow { offset: [0, 4], blur: 14, spread: 1, color: tokens.shadow };
    };
    let id = egui::Id::unique(("cell_context", app.grid.context_menu_frame));
    egui::Popup::new(id, ui.ctx().clone(), at, ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .open_bool(&mut open)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .style(style)
        .frame(crate::widgets::context_frame(ui))
        .accessible_name(lang.tr("Cell Context Menu"))
        .max_height_from_constrain(true)
        .show(|ui| {
            let height = ui.available_height();
            egui::ScrollArea::vertical().max_height(height).show(ui, |ui| {
                ui.set_width(244.0);
                cell_menu_items(app, ui);
            });
        });
    if !open {
        app.grid.context_menu = None;
    }
}

fn cell_menu_items(app: &mut SheetApp, ui: &mut egui::Ui) {
    use crate::icons::Icon;
    let link = app.session.active().and_then(|d| d.wb.active().and_then(|sh| sh.hyperlinks.get(&d.selection.active)).map(|l| l.target.clone()));
    let has_filter = app.session.active().is_some_and(|d| {
        d.wb.active()
            .is_some_and(|sh| sh.autofilter.as_ref().is_some_and(|af| !af.criteria.is_empty()) || sh.tables.iter().any(|t| !t.filter.is_empty()))
    });
    if context_choice(ui, Icon::Pen, "Edit Cell") {
        app.begin_edit(None, false);
    }
    ui.separator();
    for (icon, label, cmd) in [
        (Icon::Cut, "Cut", "edit.cut"),
        (Icon::Copy, "Copy", "edit.copy"),
        (Icon::Paste, "Paste", "edit.paste"),
        (Icon::Format, "Paste Special…", "ui:pasteSpecial"),
    ] {
        if context_choice(ui, icon, label) {
            run_cell_action(app, ui, cmd);
        }
    }
    ui.separator();
    cell_submenu(ui, Icon::Insert, "Insert", |ui| {
        cell_shift_options(app, ui, true);
        ui.separator();
        if context_choice(ui, Icon::Settings, "More Insert Options…") {
            run_cell_action(app, ui, "ui:insertCells");
        }
        if let Some((label, cmd)) = insert_copied_item(app)
            && context_choice(ui, Icon::Copy, label)
        {
            run_cell_action(app, ui, cmd);
        }
    });
    cell_submenu(ui, Icon::Delete, "Delete", |ui| {
        cell_shift_options(app, ui, false);
        ui.separator();
        if context_choice(ui, Icon::Settings, "More Delete Options…") {
            run_cell_action(app, ui, "ui:deleteCells");
        }
    });
    cell_submenu(ui, Icon::Eraser, "Clear", |ui| {
        for (icon, label, cmd) in [
            (Icon::Eraser, "Clear Contents", "edit.clearContents"),
            (Icon::Brush, "Clear Formats", "edit.clearFormats"),
            (Icon::Comment, "Clear Comments and Notes", "edit.clearComments"),
            (Icon::Link, "Clear Hyperlinks", "edit.clearHyperlinks"),
            (Icon::Delete, "Clear All", "edit.clearAll"),
        ] {
            if context_choice(ui, icon, label) {
                run_cell_action(app, ui, cmd);
            }
        }
    });
    ui.separator();
    cell_submenu(ui, Icon::SortFilter, "Sort", |ui| {
        for (icon, label, cmd) in [(Icon::SortAsc, "Sort A to Z", "data.sortAscending"), (Icon::SortDesc, "Sort Z to A", "data.sortDescending")] {
            if context_choice(ui, icon, label) {
                run_cell_action(app, ui, cmd);
            }
        }
        if context_choice(ui, Icon::SortFilter, "Custom Sort…") {
            run_cell_action(app, ui, "ui:sort");
        }
    });
    cell_submenu(ui, Icon::Filter, "Filter", |ui| {
        if context_choice(ui, Icon::Filter, "Filter by Selected Cell's Value") {
            filter_by_selected_cell(app);
        }
    });
    ui.add_enabled_ui(has_filter, |ui| {
        if context_choice(ui, Icon::Redo, "Reapply") {
            run_cell_action(app, ui, "data.reapply");
        }
    });
    if show_details_offered(app) && context_choice(ui, Icon::PivotTable, "Show Details") {
        run_cell_action(app, ui, "pivot.showDetails");
    }
    ui.separator();
    for (icon, label, cmd) in [(Icon::Comment, "New Comment", "ui:newComment"), (Icon::Note, "New Note", "ui:newNote")] {
        if context_choice(ui, icon, label) {
            run_cell_action(app, ui, cmd);
        }
    }
    ui.separator();
    if context_choice(ui, Icon::Format, "Format Cells…") {
        run_cell_action(app, ui, "ui:formatCells");
    }
    cell_submenu(ui, Icon::Comma, "Number Format", |ui| {
        for (icon, format) in [
            (Icon::Format, "General"),
            (Icon::Comma, "Number"),
            (Icon::Currency, "Currency"),
            (Icon::Percent, "Percentage"),
            (Icon::Calendar, "Short Date"),
            (Icon::TextBox, "Text"),
        ] {
            if context_choice(ui, icon, format) {
                app.run_or_alert("home.numberFormat", json!({"format": format}));
            }
        }
    });
    for (icon, label, cmd) in [
        (Icon::CondFormat, "Conditional Formatting", "ui:newFormattingRule"),
        (Icon::Validation, "Pick From Drop-down List…", "ui:pickList"),
        (Icon::Name, "Define Name…", "ui:defineName"),
    ] {
        if context_choice(ui, icon, label) {
            run_cell_action(app, ui, cmd);
        }
    }
    ui.separator();
    if context_choice(ui, Icon::Fx, "Insert Function") {
        run_cell_action(app, ui, "ui:insertFunction");
    }
    if let Some(target) = link {
        if context_choice(ui, Icon::Link, "Edit Hyperlink…") {
            run_cell_action(app, ui, "ui:insertLink");
        }
        if context_choice(ui, Icon::OpenFolder, "Open Hyperlink") {
            super::interact::follow_link(app, &target);
        }
        if context_choice(ui, Icon::Eraser, "Remove Hyperlink") {
            run_cell_action(app, ui, "edit.clearHyperlinks");
        }
    } else if context_choice(ui, Icon::Link, "Link…") {
        run_cell_action(app, ui, "ui:insertLink");
    }
    ui.separator();
    cell_submenu(ui, Icon::Freeze, "Freeze Panes", |ui| {
        for (label, cmd) in [
            ("Freeze Panes", "view.freezePanes"),
            ("Freeze Top Row", "view.freezeTopRow"),
            ("Freeze First Column", "view.freezeFirstColumn"),
            ("Unfreeze Panes", "view.unfreezePanes"),
        ] {
            if context_choice(ui, Icon::Freeze, label) {
                run_cell_action(app, ui, cmd);
            }
        }
    });
}

fn cell_shift_options(app: &mut SheetApp, ui: &mut egui::Ui, insert: bool) {
    use crate::icons::Icon;
    let (command, horizontal, vertical) = if insert {
        ("home.insertCells", (Icon::Right, "Shift cells right", "right"), (Icon::Chevron, "Shift cells down", "down"))
    } else {
        ("home.deleteCells", (Icon::Left, "Shift cells left", "left"), (Icon::ChevronUp, "Shift cells up", "up"))
    };
    for (icon, label, shift) in [horizontal, vertical, (Icon::Table, "Entire row", "row"), (Icon::Borders, "Entire column", "column")] {
        if context_choice(ui, icon, label) {
            app.run_or_alert(command, json!({"shift": shift}));
        }
    }
}

fn cell_submenu(ui: &mut egui::Ui, icon: crate::icons::Icon, label: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let label = crate::i18n::current(ui.ctx()).tr(label);
    let response = crate::widgets::context_row(ui, icon, label, true);
    egui::containers::menu::SubMenu::new().show(ui, &response, |ui| {
        ui.set_width(244.0);
        contents(ui);
    });
}

fn context_choice(ui: &mut egui::Ui, icon: crate::icons::Icon, label: &str) -> bool {
    let label = crate::i18n::current(ui.ctx()).tr(label);
    if crate::widgets::context_item(ui, icon, label).clicked() {
        ui.close();
        return true;
    }
    false
}

fn run_cell_action(app: &mut SheetApp, ui: &egui::Ui, command: &str) {
    match command {
        "edit.copy" | "edit.cut" => app.copy_to_clipboard(ui.ctx(), command),
        "edit.paste" => app.paste_from_clipboard(),
        "ui:newComment" => app.open_dialog("comment", json!({"threaded": true})),
        "ui:newNote" => app.open_dialog("comment", json!({"threaded": false})),
        _ => {
            if let Some(dialog) = command.strip_prefix("ui:") {
                app.open_dialog(dialog, json!({}));
            } else {
                app.run_or_alert(command, json!({}));
            }
        }
    }
}

fn filter_by_selected_cell(app: &mut SheetApp) {
    let Some(d) = app.session.active() else { return };
    let Some(sh) = d.wb.active() else { return };
    let cell = d.selection.active;
    let mut params = gridcraft_engine::cmd::data::filter_by_cell_params(&d.wb, sh, cell, &col_to_letters(cell.col));
    if let Some(table) = sh.table_at(cell).filter(|t| t.header_row) {
        params["table"] = json!(table.name);
    }
    app.run_or_alert("data.filterBy", params);
}

/// Whether the cell menu offers Show Details (Excel's right-click › Show Details): the active cell
/// is a value, subtotal or grand total cell of a PivotTable, or a cell of a report that is out of
/// date, where running the command says so. Labels and headers of a current report don't offer it,
/// and no cell does while the PivotTable's Enable show details option is off.
fn show_details_offered(app: &mut SheetApp) -> bool {
    let Some(d) = app.session.active() else { return false };
    let (si, at) = (d.wb.active_sheet(), d.selection.active);
    let Some((pi, pt)) = d.wb.active().and_then(|sh| sh.pivots.iter().enumerate().find(|(_, pt)| pt.last_range.is_some_and(|r| r.contains(at))))
    else {
        return false;
    };
    if !pt.enable_show_details {
        return false;
    }
    let origin = pt.last_range.map(|r| r.start).unwrap_or_default();
    let cell = (at.row.saturating_sub(origin.row), at.col.saturating_sub(origin.col));
    let key = (d.uid, d.revision, si, pi);
    if let Some((k, cells)) = &app.grid.show_details_cells
        && *k == key
    {
        return cells.as_ref().is_none_or(|c| c.contains(&cell));
    }
    let cells = gridcraft_engine::pivot::detail_cells(&d.wb, pt, si).ok();
    let on = cells.as_ref().is_none_or(|c| c.contains(&cell));
    app.grid.show_details_cells = Some((key, cells));
    on
}

/// The ways to fill a dragged range, as the fill menu and the Auto Fill Options button list them.
const FILL_MODES: [(&str, &str); 4] =
    [("Copy Cells", "copy"), ("Fill Series", "extend"), ("Fill Formatting Only", "formats"), ("Fill Without Formatting", "values")];

/// The ways to fill from a date, which both menus add after [`FILL_MODES`] when the source starts
/// with one.
const DATE_FILL_MODES: [(&str, &str); 4] = [("Fill Days", "days"), ("Fill Weekdays", "weekdays"), ("Fill Months", "months"), ("Fill Years", "years")];

/// The fill modes the menus list for a fill from `src` on the active sheet.
fn fill_modes(app: &SheetApp, src: RangeRef) -> Vec<(&'static str, &'static str)> {
    let dates = app.session.active().is_some_and(|d| gridcraft_engine::fill::starts_with_date(&d.wb, d.wb.active_sheet(), src));
    FILL_MODES.into_iter().chain(DATE_FILL_MODES.into_iter().filter(|_| dates)).collect()
}

/// Linear Trend and Growth Trend, which the right-drag fill menu adds after the other fill modes
/// when they apply to a fill from `src` to `target` on the active sheet.
fn trend_fill_modes(app: &mut SheetApp, src: RangeRef, target: RangeRef) -> Vec<(&'static str, &'static str)> {
    let Some(d) = app.session.active() else { return Vec::new() };
    let key = (d.uid, d.revision, d.wb.active_sheet(), src, target);
    if let Some((k, modes)) = &app.grid.fill_menu_trends
        && *k == key
    {
        return modes.clone();
    }
    let (linear, growth) = gridcraft_engine::fill::trends_that_apply(&d.wb, d.wb.active_sheet(), src, target);
    let modes: Vec<(&'static str, &'static str)> = [("Linear Trend", "linearTrend", linear), ("Growth Trend", "growthTrend", growth)]
        .into_iter()
        .filter(|(_, _, applies)| *applies)
        .map(|(label, mode, _)| (label, mode))
        .collect();
    app.grid.fill_menu_trends = Some((key, modes.clone()));
    modes
}

/// Remembers the fill just made so the Auto Fill Options button can offer the other ways.
pub(super) fn set_fill_tag(app: &mut SheetApp, src: RangeRef, target: RangeRef) {
    app.grid.fill_tag =
        app.session.active().map(|d| FillTag { src, target, uid: d.uid, sheet: d.wb.active_sheet(), revision: d.revision, undo_len: d.undo.len() });
}

/// The Auto Fill Options button below the bottom-right corner of cells a left-drag of the fill
/// handle just filled. Its menu redoes that fill as Copy Cells, Fill Series, Fill Formatting Only
/// or Fill Without Formatting, and from a date also as Fill Days, Fill Weekdays, Fill Months or
/// Fill Years: it undoes the drag's fill, then runs `edit.autoFill` in that mode.
/// The button goes away as soon as anything else changes the workbook or the selection moves.
pub(super) fn fill_options_tag(app: &mut SheetApp, ui: &mut egui::Ui, geo: &Geo, sh: &Sheet) {
    let Some(tag) = app.grid.fill_tag.clone() else { return };
    let current = app.session.active().is_some_and(|d| {
        d.uid == tag.uid
            && d.wb.active_sheet() == tag.sheet
            && d.revision == tag.revision
            && d.undo.len() == tag.undo_len
            && d.selection.ranges.len() == 1
            && d.selection.ranges.first().is_some_and(|r| *r == tag.target)
    });
    if !current {
        app.grid.fill_tag = None;
        return;
    }
    if app.editor.is_some() || app.dialog.is_some() || app.grid.drag != Drag::None {
        return;
    }
    let size = vec2(18.0, 18.0);
    let corner = geo.range_rect(sh, tag.target).right_bottom();
    let button = geo.mirror_rect(Rect::from_min_size(corner + vec2(2.0, 2.0), size));
    // `button` is in screen space, so compare it with the mirrored cells area.
    if !geo.mirror_rect(geo.cells).contains_rect(button) {
        return;
    }
    let lang = crate::i18n::current(ui.ctx());
    let resp = ui.interact(button, egui::Id::unique("gridcraft.fill_options_tag"), Sense::click()).on_hover_text(lang.tr("Auto Fill Options"));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, lang.tr("Auto Fill Options")));
    let t = Tokens::get(ui.ctx());
    let p = ui.painter();
    p.rect_filled(button, 3.0, if resp.hovered() { t.hover } else { t.window });
    p.rect_stroke(button, 3.0, Stroke::new(1.0, t.separator), egui::StrokeKind::Inside);
    // Two small cells and a drop-down arrow.
    let c = button.center();
    p.rect_stroke(Rect::from_min_size(pos2(c.x - 6.0, c.y - 6.0), vec2(6.0, 6.0)), 0.0, Stroke::new(1.0, t.text), egui::StrokeKind::Inside);
    p.rect_stroke(Rect::from_min_size(pos2(c.x - 3.0, c.y - 3.0), vec2(6.0, 6.0)), 0.0, Stroke::new(1.0, t.text), egui::StrokeKind::Inside);
    p.add(egui::Shape::convex_polygon(
        vec![pos2(c.x + 1.0, c.y + 5.0), pos2(c.x + 7.0, c.y + 5.0), pos2(c.x + 4.0, c.y + 8.0)],
        t.text,
        Stroke::NONE,
    ));
    let mut chosen = None;
    let modes = fill_modes(app, tag.src);
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(190.0);
        for (label, mode) in modes {
            if ui.add(egui::Button::new(lang.tr(label)).frame(false).min_size(vec2(180.0, 20.0))).clicked() {
                chosen = Some(mode);
                ui.close();
            }
        }
    });
    if let Some(mode) = chosen {
        if let Err(e) = app.run("edit.undo", json!({})) {
            app.grid.fill_tag = None;
            app.message = Some(("GridCraft".into(), crate::clean_error(&e)));
            return;
        }
        match app.run("edit.autoFill", json!({"source": tag.src.a1(), "target": tag.target.a1(), "mode": mode})) {
            Ok(_) => set_fill_tag(app, tag.src, tag.target),
            Err(e) => {
                app.grid.fill_tag = None;
                app.message = Some(("GridCraft".into(), crate::clean_error(&e)));
            }
        }
    }
}

/// The menu a right-drag of the fill handle opens on release, as in Excel.
pub(super) fn fill_menu(app: &mut SheetApp, ui: &mut egui::Ui, was_open: bool) {
    let Some((src, target, at)) = app.grid.fill_menu else { return };
    let mut close = false;
    let mut items = fill_modes(app, src);
    items.extend(trend_fill_modes(app, src, target));
    items.extend([("-", ""), ("Series…", "ui:series")]);
    let response = egui::Area::new(egui::Id::unique("fill_menu")).fixed_pos(at).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_min_width(190.0);
            let lang = crate::i18n::current(ui.ctx());
            for (label, mode) in items {
                if label == "-" {
                    ui.separator();
                    continue;
                }
                if ui.add(egui::Button::new(lang.tr(label)).frame(false).min_size(vec2(180.0, 20.0))).clicked() {
                    close = true;
                    if mode == "ui:series" {
                        let _ = app.session.run("selection.set", json!({"range": target.union(&src).a1(), "active": src.start.a1()}));
                        let direction = if target.width() != src.width() { "rows" } else { "columns" };
                        app.open_dialog("series", json!({"direction": direction}));
                    } else if let Err(e) = app.run("edit.autoFill", json!({"source": src.a1(), "target": target.a1(), "mode": mode})) {
                        app.message = Some(("GridCraft".into(), crate::clean_error(&e)));
                    }
                }
            }
        });
    });
    if close || popup_dismissed(&response.response, was_open) {
        app.grid.fill_menu = None;
    }
}

/// The right-click menu of a chart, picture, shape, slicer or form control (Edit Text and Assign
/// Action for a button, and Format Control, before Delete).
pub(super) fn object_menu(app: &mut SheetApp, ui: &mut egui::Ui) {
    let Some((at, kind, id)) = app.grid.object_menu else { return };
    let mut close = false;
    let resp = egui::Area::new(egui::Id::unique("object_context")).fixed_pos(at).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_min_width(200.0);
            let mut items = vec![];
            if kind == "control" {
                if crate::control_ui::is_button(app, id) {
                    items.extend([("Edit Text", "ui:editText"), ("Assign Action…", "ui:assignAction")]);
                }
                items.extend([("Format Control…", "ui:formatControl"), ("-", "")]);
            }
            items.push(("Delete", "object.delete"));
            if kind == "chart" {
                items.extend([("-", ""), ("Select Data…", "ui:selectData"), ("Format Chart Area…", "ui:formatChart")]);
            }
            let lang = crate::i18n::current(ui.ctx());
            for (label, action) in items {
                if label == "-" {
                    ui.separator();
                    continue;
                }
                if ui.add(egui::Button::new(lang.tr(label)).frame(false).min_size(vec2(190.0, 20.0))).clicked() {
                    close = true;
                    match action {
                        "ui:selectData" => app.open_dialog("selectData", json!({"chart": id})),
                        "ui:formatChart" => app.grid.pane = Some("formatChart".into()),
                        "ui:editText" => app.open_dialog("formatControl", json!({"id": id, "textOnly": true})),
                        "ui:assignAction" => app.open_dialog("assignAction", json!({"id": id})),
                        "ui:formatControl" => app.open_dialog("formatControl", json!({"id": id})),
                        _ => {
                            app.run_or_alert("object.delete", json!({"kind": kind, "id": id}));
                            app.selected_object = None;
                        }
                    }
                }
            }
        });
    });
    if close
        || crate::framework::escape_pressed(ui.ctx())
        || (ui.input(|i| i.pointer.any_pressed())
            && !resp.response.contains_pointer()
            && !resp.response.hovered()
            && ui.input(|i| i.pointer.interact_pos()).is_some_and(|p| !resp.response.rect.contains(p)))
    {
        app.grid.object_menu = None;
    }
}

/// The warning button beside an active cell holding a number stored as text, as in Excel: its
/// menu names the problem and offers Convert to Number for the selection (#1994).
pub(super) fn error_check_tag(app: &mut SheetApp, ui: &mut egui::Ui, geo: &Geo, sh: &Sheet) {
    if app.editor.is_some() || app.dialog.is_some() || sh.show_formulas || app.grid.drag != Drag::None {
        return;
    }
    let Some(active) = app.session.active().map(|d| d.selection.active) else { return };
    if sh.cell(active).and_then(gridcraft_engine::cmd::formulas::number_stored_as_text).is_none() {
        return;
    }
    let cell = geo.cell_rect(sh, active);
    let size = vec2(18.0, 18.0);
    // Left of the cell, or right of it when that would cover the row headings.
    let left = if cell.left() - size.x - 2.0 >= geo.cells.left() { cell.left() - size.x - 2.0 } else { cell.right() + 2.0 };
    let tag = geo.mirror_rect(Rect::from_min_size(pos2(left, cell.top()), size));
    if !geo.cells.intersects(tag) {
        return;
    }
    let lang = crate::i18n::current(ui.ctx());
    let resp = ui.interact(tag, egui::Id::unique("gridcraft.error_check_tag"), Sense::click()).on_hover_text(lang.tr("Number Stored as Text"));
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, lang.tr("Error checking options")));
    let t = Tokens::get(ui.ctx());
    let p = ui.painter();
    p.rect_filled(tag, 3.0, if resp.hovered() { t.hover } else { t.window });
    p.rect_stroke(tag, 3.0, Stroke::new(1.0, t.separator), egui::StrokeKind::Inside);
    // A warning sign: an amber diamond with an exclamation mark.
    let c = tag.center();
    let r = 6.0;
    p.add(egui::Shape::convex_polygon(
        vec![pos2(c.x, c.y - r), pos2(c.x + r, c.y), pos2(c.x, c.y + r), pos2(c.x - r, c.y)],
        t.warning_fill,
        Stroke::NONE,
    ));
    p.text(c, egui::Align2::CENTER_CENTER, "!", egui::FontId::proportional(10.0), t.warning_ink);
    egui::Popup::menu(&resp).show(|ui| {
        ui.label(egui::RichText::new(lang.tr("Number Stored as Text")).strong());
        ui.separator();
        if ui.button(lang.tr("Convert to Number")).clicked() {
            app.run_or_alert("formulas.convertToNumber", json!({}));
            ui.close();
        }
    });
}

/// Draws row/column header context menus with insert/delete, format, size, hide/unhide and group actions, plus Insert Copied Cells when available.
pub(super) fn header_menu(app: &mut SheetApp, ui: &mut egui::Ui, was_open: bool) {
    use crate::icons::Icon;
    use crate::widgets::{context_frame, context_item};

    let Some((at, rows)) = app.grid.header_menu else { return };
    let mut close = false;
    let response = egui::Area::new(egui::Id::unique("header_menu")).fixed_pos(at).order(egui::Order::Foreground).show(ui.ctx(), |ui| {
        context_frame(ui).show(ui, |ui| {
            let (insert, delete, dimension, fit, hide, unhide) = if rows {
                (
                    "home.insertRows",
                    "home.deleteRows",
                    ("Row Height…", "ui:rowHeight"),
                    ("AutoFit Row Height", "home.autofitRowHeight"),
                    "home.hideRows",
                    "home.unhideRows",
                )
            } else {
                (
                    "home.insertColumns",
                    "home.deleteColumns",
                    ("Column Width…", "ui:columnWidth"),
                    ("AutoFit Column Width", "home.autofitColumnWidth"),
                    "home.hideColumns",
                    "home.unhideColumns",
                )
            };
            ui.set_width(220.0);
            ui.spacing_mut().item_spacing.y = 1.0;
            let clipboard = [(Icon::Cut, "Cut", "edit.cut"), (Icon::Copy, "Copy", "edit.copy"), (Icon::Paste, "Paste", "edit.paste")];
            let insertion =
                [(Icon::Insert, "Header Menu|Insert", insert)].into_iter().chain(insert_copied_item(app).map(|(label, id)| (Icon::Paste, label, id)));
            let structure = [(Icon::Delete, "Delete", delete), (Icon::Eraser, "Clear Contents", "edit.clearContents")];
            let layout = [
                (Icon::Format, "Format Cells…", "ui:formatCells"),
                (Icon::Expand, dimension.0, dimension.1),
                (Icon::Relative, fit.0, fit.1),
                (Icon::Collapse, "Hide", hide),
                (Icon::Expand, "Unhide", unhide),
            ];
            let sort = (!rows).then_some((Icon::SortAsc, "Sort A to Z", "data.sortAscending"));
            // Separators share the same row stream as the actions so shortcut and dialog routing
            // stay in one place.
            let items = clipboard
                .into_iter()
                .map(Some)
                .chain([None])
                .chain(insertion.map(Some))
                .chain(structure.into_iter().map(Some))
                .chain([None])
                .chain(layout.into_iter().map(Some))
                .chain(sort.map(Some))
                .chain([Some((Icon::Group, "Group", "data.group"))]);
            let lang = crate::i18n::current(ui.ctx());
            for item in items {
                let Some((icon, label, id)) = item else {
                    ui.separator();
                    continue;
                };
                if context_item(ui, icon, lang.tr(label)).clicked() {
                    close = true;
                    match id {
                        "ui:insertPaste" => app.open_dialog("insertPaste", json!({})),
                        "ui:rowHeight" => app.open_dialog("rowHeight", json!({})),
                        "ui:formatCells" => app.open_dialog("formatCells", json!({})),
                        "ui:columnWidth" => app.open_dialog("columnWidth", json!({})),
                        "edit.copy" | "edit.cut" => {
                            app.copy_to_clipboard(ui.ctx(), id);
                        }
                        "edit.paste" => app.paste_from_clipboard(),
                        other => app.run_or_alert(other, json!({})),
                    }
                }
            }
        });
    });
    if close || popup_dismissed(&response.response, was_open) {
        app.grid.header_menu = None;
    }
}

/// The context-menu entry for Insert Copied Cells / Insert Cut Cells, when the clipboard holds a
/// copy or cut of this app: whole rows or columns insert at once, a block asks which way to shift
/// (Insert Paste). A cut block can only be pasted, so it gets no entry.
fn insert_copied_item(app: &SheetApp) -> Option<(&'static str, &'static str)> {
    let clip = app.session.clipboard.as_ref()?;
    let lines = clip.selected.is_full_rows() != clip.selected.is_full_cols();
    let cut = clip.cut && app.session.active().is_some_and(|d| d.uid == clip.doc_uid);
    match (cut, lines) {
        (true, true) => Some(("Insert Cut Cells", "home.insertCopiedCells")),
        (true, false) => None,
        (false, true) => Some(("Insert Copied Cells", "home.insertCopiedCells")),
        (false, false) => Some(("Insert Copied Cells…", "ui:insertPaste")),
    }
}

pub(super) fn is_checkbox(sh: &Sheet, app: &SheetApp) -> bool {
    let Some(d) = app.session.active() else { return false };
    d.wb.styles.get(sh.style_id(d.selection.active)).num_fmt.as_str() == "checkbox"
}
