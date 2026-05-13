use crate::engine::player::signed_effect_text;
use crate::engine::snapshot::{overlay_rows, GameSnapshot};
use crate::PlayerUiRow;
use slint::SharedString;

pub struct ShooterUiOptions {
    pub count_skirmishers: bool,
    pub count_archers: bool,
    pub count_xbows: bool,
    pub count_arab_bows: bool,
    pub count_slingers: bool,
}

pub fn to_player_ui_rows(
    snapshot: &GameSnapshot,
    player_names: &[SharedString],
    visible_players: &[bool],
    player_colors: &[i32],
    shooter_options: &ShooterUiOptions,
) -> Vec<PlayerUiRow> {
    overlay_rows(snapshot)
        .into_iter()
        .map(|row| {
            let slot = row.player_id.saturating_sub(1) as usize;
            let name = player_names
                .get(slot)
                .map(SharedString::as_str)
                .unwrap_or("Player");
            let visible = visible_players.get(slot).copied().unwrap_or(true);
            let color_index = player_colors
                .get(slot)
                .copied()
                .unwrap_or(row.player_id as i32);

            PlayerUiRow {
                visible,
                name: name.into(),
                color_index,
                color_name: player_color_name(color_index).into(),
                gold: row.gold,
                wood: row.wood,
                stone: row.stone,
                food_stock: row.food_stock,
                food_ratio_text: signed_effect_text_3(row.food_ratio_effect).into(),
                tax_text: signed_effect_text_3(row.tax_effect).into(),
                total_effect_text: signed_effect_text_3(row.total_popularity_effect).into(),
                popularity_text: unsigned_text_3(row.popularity).into(),
                popularity: row.popularity,
                population: row.population,
                army: row.army,
                buildings: row.buildings,
                traps: row.traps,
                ff: row.ff,
                ff_text: signed_effect_text_3(row.ff).into(),
                ha: row.ha,
                skirmishers: row.skirmishers,
                archers: row.archers,
                xbows: row.xbows,
                arab_bows: row.arab_bows,
                slingers: row.slingers,
                camel_lancers: row.camel_lancers,
                knights: row.knights,
                shields: row.shields,
                catapults: row.catapults,
                workshops: row.workshops,
                shooters: shooter_count_for_ui(&row, shooter_options),
                army_killed: row.army_killed,
                army_lost: row.army_lost,
                extreme_power: row.extreme_power,
                stat_gold_earned: row.stat_gold_earned,
                stat_food_produced: row.stat_food_produced,
                stat_wood_produced: row.stat_wood_produced,
                stat_stone_produced: row.stat_stone_produced,
                stat_iron_produced: row.stat_iron_produced,
                stat_weapons_produced: row.stat_weapons_produced,
            }
        })
        .collect()
}

fn shooter_count_for_ui(
    row: &crate::engine::snapshot::PlayerOverlayStats,
    options: &ShooterUiOptions,
) -> i32 {
    let mut total = row.ha;
    if options.count_skirmishers {
        total += row.skirmishers;
    }
    if options.count_archers {
        total += row.archers;
    }
    if options.count_xbows {
        total += row.xbows;
    }
    if options.count_arab_bows {
        total += row.arab_bows;
    }
    if options.count_slingers {
        total += row.slingers;
    }
    total
}

fn player_color_name(index: i32) -> &'static str {
    match index {
        1 => "Red",
        2 => "Orange",
        3 => "Yellow",
        4 => "Blue",
        5 => "Black",
        6 => "Violet",
        7 => "Sky",
        8 => "Green",
        _ => "None",
    }
}

fn signed_effect_text_3(value: i32) -> String {
    signed_effect_text(value.clamp(-99, 99))
}

fn unsigned_text_3(value: i32) -> String {
    value.clamp(0, 100).to_string()
}
