use std::{cell::RefCell, rc::Rc};

use slint::{Color, ComponentHandle, Image, Model, ModelRc, SharedString, VecModel, Weak};

use crate::{
    config::{AppConfig, ColumnVisibility},
    overlay_columns::{self, ColumnId, COLUMN_GAP},
    AppWindow, OverlayCellUi, OverlayColumnUi, OverlayRowUi, OverlayWindow, PlayerUiRow,
};

const DRAG_THRESHOLD: f32 = 5.0;

thread_local! {
    static ICONS: RefCell<Vec<(ColumnId, Image)>> = const { RefCell::new(Vec::new()) };
}

#[derive(Clone)]
struct DragState {
    column: ColumnId,
    start_x: f32,
    active: bool,
    visible: Vec<ColumnId>,
    widths: Vec<f32>,
    signature: SharedString,
}

struct EditorController {
    app: Weak<AppWindow>,
    overlay: Weak<OverlayWindow>,
    config: Rc<RefCell<AppConfig>>,
    drag: RefCell<Option<DragState>>,
}

/// Installs transient editing controls. The saved order includes hidden columns.
pub fn install(app: &AppWindow, overlay: &OverlayWindow, config_state: Rc<RefCell<AppConfig>>) {
    set_order(
        overlay,
        &overlay_columns::normalize_order(&config_state.borrow().overlay.column_order),
    );
    overlay.set_editing(false);
    clear_drag(overlay);

    let controller = Rc::new(EditorController {
        app: app.as_weak(),
        overlay: overlay.as_weak(),
        config: config_state,
        drag: RefCell::new(None),
    });

    overlay.on_request_editing({
        let controller = controller.clone();
        move |editing| controller.set_editing(editing)
    });
    overlay.on_select_column({
        let controller = controller.clone();
        move |key| controller.select(&key)
    });
    overlay.on_move_column({
        let controller = controller.clone();
        move |direction| controller.move_selected(direction)
    });
    overlay.on_set_column_visible({
        let controller = controller.clone();
        move |key, visible| controller.set_visible(&key, visible)
    });
    overlay.on_reset_column_order({
        let controller = controller.clone();
        move || controller.reset_order()
    });
    overlay.on_begin_column_drag({
        let controller = controller.clone();
        move |x| controller.begin_drag(x)
    });
    overlay.on_update_column_drag({
        let controller = controller.clone();
        move |x| controller.update_drag(x)
    });
    overlay.on_finish_column_drag({
        let controller = controller.clone();
        move |x, valid| controller.finish_drag(x, valid)
    });
    overlay.on_header_title_at({
        let controller = controller.clone();
        move |x| controller.header_title_at(x)
    });
    overlay.on_header_remove_at({
        let controller = controller.clone();
        move |x| controller.header_remove_at(x)
    });
    overlay.on_cancel_column_drag(move || controller.cancel_drag());

    sync_latest(overlay, app);
}

/// Headers rebuild only when layout changes; live values can update during a drag.
pub fn sync(overlay: &OverlayWindow, app: &AppWindow, rows: &[PlayerUiRow]) {
    overlay.set_column_gap(COLUMN_GAP);
    let order = current_order(overlay);
    let visibility = column_visibility(app);
    let visible = overlay_columns::visible_columns(&order, &visibility);
    let font_size = app.get_overlay_font_size();
    let signature = format!(
        "{}|{}|{}",
        order.join(","),
        visible
            .iter()
            .map(|column| column.key())
            .collect::<Vec<_>>()
            .join(","),
        font_size.to_bits()
    );

    if overlay.get_column_signature().as_str() != signature {
        // Font, visibility or external settings changes invalidate captured geometry.
        overlay.invoke_cancel_column_drag();
        overlay.set_statistics_width(
            visible
                .iter()
                .map(|column| column.width(font_size))
                .sum::<f32>()
                + COLUMN_GAP * visible.len().saturating_sub(1) as f32,
        );
        overlay.set_columns(column_model(&visible, font_size));
        let hidden: Vec<_> = order
            .iter()
            .filter_map(|key| ColumnId::parse(key))
            .filter(|column| !column.enabled(&visibility))
            .collect();
        overlay.set_hidden_columns(column_model(&hidden, font_size));
        overlay.set_column_signature(signature.into());
        if !visible
            .iter()
            .any(|column| column.key() == overlay.get_selected_column().as_str())
        {
            overlay.set_selected_column(SharedString::default());
        }
    }

    let rendered_rows: Vec<_> = rows
        .iter()
        .filter(|row| row.visible)
        .map(|row| OverlayRowUi {
            name: row.name.clone(),
            name_color: player_name_color(row.color_index),
            cells: Rc::new(VecModel::from(
                visible
                    .iter()
                    .map(|column| cell(*column, row))
                    .collect::<Vec<_>>(),
            ))
            .into(),
        })
        .collect();
    if !update_rows_in_place(&overlay.get_overlay_rows(), &rendered_rows) {
        overlay.set_overlay_rows(Rc::new(VecModel::from(rendered_rows)).into());
    }
}

fn update_rows_in_place(current: &ModelRc<OverlayRowUi>, rows: &[OverlayRowUi]) -> bool {
    let Some(model) = current.as_any().downcast_ref::<VecModel<OverlayRowUi>>() else {
        return false;
    };
    if model.row_count() != rows.len() {
        return false;
    }
    let existing: Vec<_> = model.iter().collect();
    if existing.iter().zip(rows).any(|(current, replacement)| {
        current.cells.row_count() != replacement.cells.row_count()
            || !current.cells.as_any().is::<VecModel<OverlayCellUi>>()
    }) {
        return false;
    }

    for (index, (mut current, replacement)) in existing.into_iter().zip(rows).enumerate() {
        if let Some(cells) = current
            .cells
            .as_any()
            .downcast_ref::<VecModel<OverlayCellUi>>()
        {
            for (cell_index, replacement) in replacement.cells.iter().enumerate() {
                if cells.row_data(cell_index).is_none_or(|cell| {
                    cell.text != replacement.text || cell.value_color != replacement.value_color
                }) {
                    cells.set_row_data(cell_index, replacement);
                }
            }
        }
        if current.name != replacement.name || current.name_color != replacement.name_color {
            current.name = replacement.name.clone();
            current.name_color = replacement.name_color;
            model.set_row_data(index, current);
        }
    }
    true
}

impl EditorController {
    fn windows(&self) -> Option<(AppWindow, OverlayWindow)> {
        Some((self.app.upgrade()?, self.overlay.upgrade()?))
    }

    fn header_title_at(&self, x: f32) -> SharedString {
        let Some(overlay) = self.overlay.upgrade() else {
            return SharedString::default();
        };
        let columns = overlay.get_columns();
        let widths: Vec<_> = columns.iter().map(|column| column.column_width).collect();
        hit_column(x, &widths)
            .and_then(|index| columns.row_data(index))
            .map(|column| column.full_title)
            .unwrap_or_default()
    }

    fn header_remove_at(&self, x: f32) -> SharedString {
        let Some(overlay) = self.overlay.upgrade() else {
            return SharedString::default();
        };
        let columns = overlay.get_columns();
        let widths: Vec<_> = columns.iter().map(|column| column.column_width).collect();
        hit_remove(x, &widths)
            .and_then(|index| columns.row_data(index))
            .map(|column| column.id)
            .unwrap_or_default()
    }

    fn set_editing(&self, editing: bool) {
        let Some((app, overlay)) = self.windows() else {
            return;
        };
        self.cancel_drag();
        overlay.set_editing(editing);
        overlay.set_edit_status(
            if editing {
                "Drag headers to move columns. Select a column to remove it, or add one below."
            } else {
                ""
            }
            .into(),
        );
        sync_latest(&overlay, &app);
    }

    fn select(&self, key: &str) {
        let Some((app, overlay)) = self.windows() else {
            return;
        };
        if overlay.get_editing()
            && ColumnId::parse(key).is_some_and(|column| column.enabled(&column_visibility(&app)))
        {
            overlay.set_selected_column(key.into());
        }
    }

    fn move_selected(&self, direction: i32) {
        let Some((app, overlay)) = self.windows() else {
            return;
        };
        if !overlay.get_editing() || direction == 0 {
            return;
        }
        self.cancel_drag();
        let selected = overlay.get_selected_column();
        let mut order = current_order(&overlay);
        let visible = overlay_columns::visible_columns(&order, &column_visibility(&app));
        let Some(index) = visible
            .iter()
            .position(|column| column.key() == selected.as_str())
        else {
            return;
        };
        let changed = if direction < 0 && index > 0 {
            overlay_columns::move_before(&mut order, &selected, Some(visible[index - 1].key()))
        } else if direction > 0 && index + 1 < visible.len() {
            overlay_columns::place_after(&mut order, &selected, Some(visible[index + 1].key()))
        } else {
            false
        };
        if changed {
            self.commit(&app, &overlay, order);
        }
    }

    fn set_visible(&self, key: &str, visible: bool) {
        let Some((app, overlay)) = self.windows() else {
            return;
        };
        let Some(column) = ColumnId::parse(key) else {
            return;
        };
        if !overlay.get_editing() || column.enabled(&column_visibility(&app)) == visible {
            return;
        }
        self.cancel_drag();
        let mut order = current_order(&overlay);
        if visible {
            let selected = overlay.get_selected_column();
            let after = ColumnId::parse(&selected)
                .filter(|selected| selected.enabled(&column_visibility(&app)));
            overlay_columns::place_after(&mut order, key, after.map(ColumnId::key));
        }
        let mut visibility = column_visibility(&app);
        column.set_enabled(&mut visibility, visible);
        apply_visibility(&app, &visibility);
        overlay.set_selected_column(if visible { key.into() } else { "".into() });
        self.commit(&app, &overlay, order);
    }

    fn reset_order(&self) {
        let Some((app, overlay)) = self.windows() else {
            return;
        };
        if !overlay.get_editing() {
            return;
        }
        self.cancel_drag();
        let defaults = overlay_columns::default_order();
        if defaults != current_order(&overlay) {
            self.commit(&app, &overlay, defaults);
        }
    }

    fn begin_drag(&self, x: f32) {
        let Some((app, overlay)) = self.windows() else {
            return;
        };
        self.cancel_drag();
        if !overlay.get_editing() || !x.is_finite() {
            return;
        }
        let visible =
            overlay_columns::visible_columns(&current_order(&overlay), &column_visibility(&app));
        let widths: Vec<_> = visible
            .iter()
            .map(|column| column.width(app.get_overlay_font_size()))
            .collect();
        let Some(index) = hit_column(x, &widths) else {
            return;
        };
        let column = visible[index];
        overlay.set_selected_column(column.key().into());
        *self.drag.borrow_mut() = Some(DragState {
            column,
            start_x: x,
            active: false,
            visible,
            widths,
            signature: overlay.get_column_signature(),
        });
    }

    fn update_drag(&self, x: f32) {
        let Some((_, overlay)) = self.windows() else {
            return;
        };
        if !overlay.get_editing() || !x.is_finite() {
            self.cancel_drag();
            return;
        }
        let mut state = self.drag.borrow_mut();
        let Some(drag) = state.as_mut() else {
            return;
        };
        if drag.signature != overlay.get_column_signature() {
            *state = None;
            clear_drag(&overlay);
            return;
        }
        if !drag.active && (x - drag.start_x).abs() < DRAG_THRESHOLD {
            return;
        }
        drag.active = true;
        overlay.set_dragging(true);
        overlay.set_drag_label(drag.column.label().into());
        overlay.set_drag_icon(icon(drag.column));
        overlay.set_drop_position(insertion_target(x, &drag.widths).1);
    }

    fn finish_drag(&self, x: f32, valid: bool) {
        let Some((app, overlay)) = self.windows() else {
            return;
        };
        let state = self.drag.borrow_mut().take();
        clear_drag(&overlay);
        let Some(drag) = state else {
            return;
        };
        if !valid
            || !drag.active
            || !x.is_finite()
            || !overlay.get_editing()
            || drag.signature != overlay.get_column_signature()
        {
            return;
        }
        let (target, _) = insertion_target(x, &drag.widths);
        let before = drag.visible.get(target).map(|column| column.key());
        let mut order = current_order(&overlay);
        if overlay_columns::move_before(&mut order, drag.column.key(), before) {
            self.commit(&app, &overlay, order);
        }
    }

    fn cancel_drag(&self) {
        *self.drag.borrow_mut() = None;
        if let Some(overlay) = self.overlay.upgrade() {
            clear_drag(&overlay);
        }
    }

    fn commit(&self, app: &AppWindow, overlay: &OverlayWindow, order: Vec<String>) {
        set_order(overlay, &order);
        // Keep the new in-memory state even if the disk is temporarily unwritable.
        let mut config = self.config.borrow().clone();
        config.overlay.column_order = order;
        config.columns = column_visibility(app);
        let result = config.save();
        *self.config.borrow_mut() = config;
        sync_latest(overlay, app);
        overlay.set_edit_status(match result {
            Ok(()) => {
                "Columns saved. Drag headers, add columns, or remove the selected column.".into()
            }
            Err(error) => {
                format!("Columns changed, but settings could not be saved: {error:#}").into()
            }
        });
    }
}

fn clear_drag(overlay: &OverlayWindow) {
    overlay.set_dragging(false);
    overlay.set_drag_label(SharedString::default());
    overlay.set_drop_position(-1.0);
}

fn sync_latest(overlay: &OverlayWindow, app: &AppWindow) {
    let model = overlay.get_player_rows();
    let rows: Vec<_> = (0..model.row_count())
        .filter_map(|index| model.row_data(index))
        .collect();
    sync(overlay, app, &rows);
}

fn current_order(overlay: &OverlayWindow) -> Vec<String> {
    let model = overlay.get_column_order();
    let keys: Vec<_> = (0..model.row_count())
        .filter_map(|index| model.row_data(index))
        .map(|key| key.to_string())
        .collect();
    overlay_columns::normalize_order(&keys)
}

fn set_order(overlay: &OverlayWindow, order: &[String]) {
    overlay.set_column_order(
        Rc::new(VecModel::from(
            order
                .iter()
                .map(|key| key.as_str().into())
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
}

fn column_model(columns: &[ColumnId], font_size: f32) -> ModelRc<OverlayColumnUi> {
    Rc::new(VecModel::from(
        columns
            .iter()
            .map(|column| OverlayColumnUi {
                id: column.key().into(),
                title: column.label().into(),
                full_title: column.full_label().into(),
                group: column.group().into(),
                icon: icon(*column),
                column_width: column.width(font_size),
            })
            .collect::<Vec<_>>(),
    ))
    .into()
}

fn hit_column(x: f32, widths: &[f32]) -> Option<usize> {
    let mut left = 0.0;
    for (index, width) in widths.iter().enumerate() {
        if x >= left && x < left + width {
            return Some(index);
        }
        left += width + COLUMN_GAP;
    }
    None
}

fn hit_remove(x: f32, widths: &[f32]) -> Option<usize> {
    let index = hit_column(x, widths)?;
    let right = widths[..=index].iter().sum::<f32>() + COLUMN_GAP * index as f32;
    (x >= right - 18.0).then_some(index)
}

fn insertion_target(x: f32, widths: &[f32]) -> (usize, f32) {
    let mut left = 0.0;
    for (index, width) in widths.iter().enumerate() {
        if x < left + width / 2.0 {
            return (
                index,
                if index == 0 {
                    0.0
                } else {
                    left - COLUMN_GAP / 2.0
                },
            );
        }
        left += width + COLUMN_GAP;
    }
    (widths.len(), (left - COLUMN_GAP).max(0.0))
}

fn column_visibility(app: &AppWindow) -> ColumnVisibility {
    ColumnVisibility {
        gold: app.get_show_gold(),
        population: app.get_show_population(),
        army: app.get_show_army(),
        food_effect: app.get_show_food_effect(),
        tax_effect: app.get_show_tax_effect(),
        ha: app.get_show_ha(),
        cl: app.get_show_cl(),
        shooters: app.get_show_shooters(),
        acquired_gold: app.get_show_acq_gold(),
        produced_weapons: app.get_show_prod_weapons(),
        produced_food: app.get_show_prod_food(),
        produced_stone: app.get_show_prod_stone(),
        produced_iron: app.get_show_prod_iron(),
        produced_wood: app.get_show_prod_wood(),
        total_effect: app.get_show_total_effect(),
        knights: app.get_show_knights(),
        fear_factor: app.get_show_ff(),
        army_killed: app.get_show_army_killed(),
        army_lost: app.get_show_army_lost(),
    }
}

fn apply_visibility(app: &AppWindow, columns: &ColumnVisibility) {
    app.set_show_population(columns.population);
    app.set_show_food_effect(columns.food_effect);
    app.set_show_tax_effect(columns.tax_effect);
    app.set_show_ff(columns.fear_factor);
    app.set_show_total_effect(columns.total_effect);
    app.set_show_gold(columns.gold);
    app.set_show_acq_gold(columns.acquired_gold);
    app.set_show_ha(columns.ha);
    app.set_show_army(columns.army);
    app.set_show_cl(columns.cl);
    app.set_show_shooters(columns.shooters);
    app.set_show_knights(columns.knights);
    app.set_show_prod_food(columns.produced_food);
    app.set_show_prod_weapons(columns.produced_weapons);
    app.set_show_prod_stone(columns.produced_stone);
    app.set_show_prod_iron(columns.produced_iron);
    app.set_show_prod_wood(columns.produced_wood);
    app.set_show_army_killed(columns.army_killed);
    app.set_show_army_lost(columns.army_lost);
}

fn cell(column: ColumnId, row: &PlayerUiRow) -> OverlayCellUi {
    let white = Color::from_rgb_u8(255, 255, 255);
    let (text, value_color) = match column {
        ColumnId::Population => (
            row.population.to_string(),
            Color::from_rgb_u8(0x7C, 0xCB, 0xEE),
        ),
        ColumnId::FoodEffect => (row.food_ratio_text.to_string(), white),
        ColumnId::TaxEffect => (row.tax_text.to_string(), white),
        ColumnId::FearFactor => (row.ff_text.to_string(), fear_factor_color(row.ff)),
        ColumnId::Popularity => (row.popularity_text.to_string(), row.popularity_color),
        ColumnId::Gold => (row.gold.to_string(), Color::from_rgb_u8(0xF2, 0xC7, 0x5C)),
        ColumnId::AcquiredGold => (
            row.stat_gold_earned.to_string(),
            Color::from_rgb_u8(0xF2, 0xC7, 0x5C),
        ),
        ColumnId::HorseArchers => (row.ha.to_string(), white),
        ColumnId::Army => (row.army.to_string(), Color::from_rgb_u8(0xC5, 0xA3, 0xF1)),
        ColumnId::CamelLancers => (row.camel_lancers.to_string(), white),
        ColumnId::Shooters => (row.shooters.to_string(), white),
        ColumnId::Knights => (row.knights.to_string(), white),
        ColumnId::ProducedFood => (row.stat_food_produced.to_string(), white),
        ColumnId::ProducedWeapons => (row.stat_weapons_produced.to_string(), white),
        ColumnId::ProducedStone => (row.stat_stone_produced.to_string(), white),
        ColumnId::ProducedIron => (row.stat_iron_produced.to_string(), white),
        ColumnId::ProducedWood => (row.stat_wood_produced.to_string(), white),
        ColumnId::ArmyKilled => (
            row.army_killed.to_string(),
            Color::from_rgb_u8(0x76, 0xD2, 0x9A),
        ),
        ColumnId::ArmyLost => (
            row.army_lost.to_string(),
            Color::from_rgb_u8(0xF2, 0x8C, 0x88),
        ),
    };
    OverlayCellUi {
        text: text.into(),
        value_color,
    }
}

fn fear_factor_color(value: i32) -> Color {
    let value = value.clamp(-5, 5);
    let target = if value < 0 {
        [0xF2, 0x8C, 0x88]
    } else {
        [0x76, 0xD2, 0x9A]
    };
    let amount = value.abs();
    let channel = |index: usize| ((255 * (5 - amount) + target[index] * amount + 2) / 5) as u8;
    Color::from_rgb_u8(channel(0), channel(1), channel(2))
}

fn player_name_color(index: i32) -> Color {
    let [red, green, blue] = match index {
        1 => [0xDF, 0x3B, 0x30],
        2 => [0xF2, 0x8C, 0x28],
        3 => [0xF2, 0xD4, 0x47],
        4 => [0x3B, 0x6E, 0xEA],
        5 => [0xFF, 0xFF, 0xFF],
        6 => [0x8E, 0x44, 0xD7],
        7 => [0x58, 0xC7, 0xE8],
        8 => [0x33, 0xA8, 0x52],
        _ => [0xCB, 0xD5, 0xE1],
    };
    Color::from_rgb_u8(red, green, blue)
}

fn icon(column: ColumnId) -> Image {
    ICONS.with(|icons| {
        let mut icons = icons.borrow_mut();
        if let Some((_, image)) = icons.iter().find(|(cached, _)| *cached == column) {
            return image.clone();
        }
        let data: &[u8] = match column {
            ColumnId::Population => include_bytes!("../../assets/overlay-icons/population.svg"),
            ColumnId::FoodEffect => include_bytes!("../../assets/overlay-icons/food-ration.svg"),
            ColumnId::TaxEffect => include_bytes!("../../assets/overlay-icons/tax.svg"),
            ColumnId::FearFactor => include_bytes!("../../assets/overlay-icons/fear-factor.svg"),
            ColumnId::Popularity => include_bytes!("../../assets/overlay-icons/popularity.svg"),
            ColumnId::Gold => include_bytes!("../../assets/overlay-icons/gold.svg"),
            ColumnId::AcquiredGold => {
                include_bytes!("../../assets/overlay-icons/gold-acquired.svg")
            }
            ColumnId::HorseArchers => include_bytes!("../../assets/overlay-icons/horse-archer.svg"),
            ColumnId::Army => include_bytes!("../../assets/overlay-icons/army.svg"),
            ColumnId::CamelLancers => include_bytes!("../../assets/overlay-icons/camel-lancer.svg"),
            ColumnId::Shooters => include_bytes!("../../assets/overlay-icons/shooters.svg"),
            ColumnId::Knights => include_bytes!("../../assets/overlay-icons/knight.svg"),
            ColumnId::ProducedFood => {
                include_bytes!("../../assets/overlay-icons/food-produced.svg")
            }
            ColumnId::ProducedWeapons => {
                include_bytes!("../../assets/overlay-icons/weapons-produced.svg")
            }
            ColumnId::ProducedStone => {
                include_bytes!("../../assets/overlay-icons/stone-produced.svg")
            }
            ColumnId::ProducedIron => {
                include_bytes!("../../assets/overlay-icons/iron-produced.svg")
            }
            ColumnId::ProducedWood => {
                include_bytes!("../../assets/overlay-icons/wood-produced.svg")
            }
            ColumnId::ArmyKilled => include_bytes!("../../assets/overlay-icons/army-killed.svg"),
            ColumnId::ArmyLost => include_bytes!("../../assets/overlay-icons/army-lost.svg"),
        };
        let image = Image::load_from_svg_data(data).unwrap_or_default();
        icons.push((column, image.clone()));
        image
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fear_factor_palette_has_neutral_and_exact_endpoints() {
        assert_eq!(fear_factor_color(-5), Color::from_rgb_u8(0xF2, 0x8C, 0x88));
        assert_eq!(fear_factor_color(0), Color::from_rgb_u8(255, 255, 255));
        assert_eq!(fear_factor_color(5), Color::from_rgb_u8(0x76, 0xD2, 0x9A));
        assert_eq!(fear_factor_color(i32::MIN), fear_factor_color(-5));
        assert_eq!(fear_factor_color(i32::MAX), fear_factor_color(5));
        for value in -5..5 {
            assert_ne!(fear_factor_color(value), fear_factor_color(value + 1));
        }
    }

    #[test]
    fn header_hit_testing_ignores_gaps_and_player_area() {
        let widths = [60.0, 90.0, 40.0];
        assert_eq!(hit_column(-1.0, &widths), None);
        assert_eq!(hit_column(0.0, &widths), Some(0));
        assert_eq!(hit_column(59.9, &widths), Some(0));
        assert_eq!(hit_column(60.0, &widths), None);
        assert_eq!(hit_column(65.9, &widths), None);
        assert_eq!(hit_column(66.0, &widths), Some(1));
        assert_eq!(hit_column(162.0, &widths), Some(2));
        assert_eq!(hit_column(202.0, &widths), None);
    }

    #[test]
    fn insertion_uses_column_centers_and_markers_between_columns() {
        let widths = [60.0, 90.0, 40.0];
        assert_eq!(insertion_target(-50.0, &widths), (0, 0.0));
        assert_eq!(insertion_target(29.9, &widths), (0, 0.0));
        assert_eq!(insertion_target(30.0, &widths), (1, 63.0));
        assert_eq!(insertion_target(111.0, &widths), (2, 159.0));
        assert_eq!(insertion_target(182.0, &widths), (3, 202.0));
        assert_eq!(insertion_target(500.0, &widths), (3, 202.0));
        assert_eq!(insertion_target(0.0, &[]), (0, 0.0));
    }

    #[test]
    fn remove_hit_testing_uses_only_the_last_eighteen_pixels_and_skips_gaps() {
        let widths = [60.0, 90.0, 40.0];
        for (x, expected) in [
            (-1.0, None),
            (41.9, None),
            (42.0, Some(0)),
            (59.9, Some(0)),
            (60.0, None),
            (65.9, None),
            (66.0, None),
            (137.9, None),
            (138.0, Some(1)),
            (155.9, Some(1)),
            (156.0, None),
            (183.9, None),
            (184.0, Some(2)),
            (201.9, Some(2)),
            (202.0, None),
            (f32::NAN, None),
            (f32::INFINITY, None),
        ] {
            assert_eq!(hit_remove(x, &widths), expected);
        }
    }

    #[test]
    fn compact_drag_geometry_matches_the_ordered_column_widths_at_every_font() {
        let mut order = overlay_columns::default_order();
        overlay_columns::move_before(&mut order, "gold", Some("population"));
        let visible = overlay_columns::visible_columns(&order, &ColumnVisibility::default());
        assert_eq!(visible.first(), Some(&ColumnId::Gold));
        for font in [10.0, 16.0, 26.0] {
            let widths: Vec<_> = visible.iter().map(|column| column.width(font)).collect();
            let mut left = 0.0;
            for (index, width) in widths.iter().enumerate() {
                let center = left + width / 2.0;
                assert_eq!(hit_column(center, &widths), Some(index));
                let (target, marker) = insertion_target(center, &widths);
                assert_eq!(target, index + 1);
                let expected = left
                    + width
                    + if index + 1 == widths.len() {
                        0.0
                    } else {
                        COLUMN_GAP / 2.0
                    };
                assert!((marker - expected).abs() < 0.001);
                left += width + COLUMN_GAP;
            }
        }
    }

    #[test]
    fn selected_column_value_tracks_registry_identity() {
        let row = PlayerUiRow {
            gold: 876,
            population: 123,
            ff: -5,
            ff_text: "-5".into(),
            ..PlayerUiRow::default()
        };
        assert_eq!(cell(ColumnId::Gold, &row).text.as_str(), "876");
        assert_eq!(cell(ColumnId::Population, &row).text.as_str(), "123");
        assert_eq!(
            cell(ColumnId::FearFactor, &row).value_color,
            fear_factor_color(-5)
        );
    }

    #[test]
    fn live_updates_retain_row_and_cell_models_for_value_hover() {
        let old_cells: ModelRc<OverlayCellUi> = Rc::new(VecModel::from(vec![OverlayCellUi {
            text: "99".into(),
            value_color: Color::from_rgb_u8(255, 255, 255),
        }]))
        .into();
        let current: ModelRc<OverlayRowUi> = Rc::new(VecModel::from(vec![OverlayRowUi {
            name: "Before".into(),
            name_color: Color::from_rgb_u8(255, 255, 255),
            cells: old_cells.clone(),
        }]))
        .into();
        let replacement = vec![OverlayRowUi {
            name: "After".into(),
            name_color: Color::from_rgb_u8(0x76, 0xD2, 0x9A),
            cells: Rc::new(VecModel::from(vec![OverlayCellUi {
                text: "1234567890".into(),
                value_color: fear_factor_color(-5),
            }]))
            .into(),
        }];

        assert!(update_rows_in_place(&current, &replacement));
        let updated = current.row_data(0).unwrap();
        assert_eq!(updated.name, "After");
        assert_eq!(updated.name_color, replacement[0].name_color);
        assert_eq!(updated.cells, old_cells);
        assert_eq!(old_cells.row_data(0).unwrap().text, "1234567890");
        assert_eq!(
            old_cells.row_data(0).unwrap().value_color,
            fear_factor_color(-5)
        );
        assert!(update_rows_in_place(&current, &replacement));
        assert_eq!(current.row_data(0).unwrap().cells, old_cells);
    }

    #[test]
    fn structure_changes_request_a_new_model_without_partially_updating_rows() {
        let current: ModelRc<OverlayRowUi> = Rc::new(VecModel::from(vec![OverlayRowUi {
            name: "Preserved".into(),
            cells: Rc::new(VecModel::from(vec![OverlayCellUi {
                text: "99".into(),
                ..OverlayCellUi::default()
            }]))
            .into(),
            ..OverlayRowUi::default()
        }]))
        .into();
        assert!(!update_rows_in_place(&current, &[]));
        assert!(!update_rows_in_place(
            &current,
            &[OverlayRowUi {
                name: "Should not replace".into(),
                cells: Rc::new(VecModel::from(Vec::<OverlayCellUi>::new())).into(),
                ..OverlayRowUi::default()
            }]
        ));
        assert_eq!(current.row_data(0).unwrap().name, "Preserved");
        assert_eq!(
            current.row_data(0).unwrap().cells.row_data(0).unwrap().text,
            "99"
        );
    }
}
