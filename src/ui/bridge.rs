use crate::engine::player::signed_effect_text;
use crate::engine::snapshot::{overlay_rows, GameSnapshot};
use crate::PlayerUiRow;
use slint::{Color, SharedString};

const POPULARITY_BASE_RATES: [i32; 21] = [
    -8, -6, -5, -4, -3, -3, -2, -2, -1, -1, 10, 12, 14, 16, 20, 24, 28, 32, 36, 40, 40,
];

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
            let popularity = displayed_popularity(row.popularity);
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
                popularity_text: unsigned_text_3(popularity).into(),
                popularity,
                popularity_color: popularity_color(row.popularity),
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

fn displayed_popularity(raw: i32) -> i32 {
    // The game stores popularity in hundredths of a point.
    (raw.clamp(0, 10_000) + 50) / 100
}

fn popularity_color(raw: i32) -> Color {
    // Use unrounded hundredths; doubling positive rates leaves normalized colors unchanged.
    let band = (raw.clamp(0, 10_000) / 500) as usize;
    spawn_rate_color(POPULARITY_BASE_RATES[band])
}

fn spawn_rate_color(rate: i32) -> Color {
    let rate = rate.clamp(0, 40);
    let (low_rate, high_rate, low, high) = match rate {
        0..=10 => (0, 10, [0xF2, 0x8C, 0x88], [0xEC, 0xA3, 0x6A]),
        11..=20 => (10, 20, [0xEC, 0xA3, 0x6A], [0xE3, 0xCB, 0x68]),
        _ => (20, 40, [0xE3, 0xCB, 0x68], [0x76, 0xD2, 0x9A]),
    };
    let span = high_rate - low_rate;
    let position = rate - low_rate;
    let channel = |index: usize| {
        ((low[index] * (span - position) + high[index] * position + span / 2) / span) as u8
    };
    Color::from_rgb_u8(channel(0), channel(1), channel(2))
}

#[cfg(test)]
mod tests {
    use super::{displayed_popularity, popularity_color, spawn_rate_color};
    use slint::Color;

    #[test]
    fn popularity_rounds_to_game_display_scale() {
        for (raw, expected) in [
            (0, 0),
            (49, 0),
            (50, 1),
            (7_549, 75),
            (7_550, 76),
            (9_999, 100),
            (10_000, 100),
            (-1, 0),
            (i32::MAX, 100),
        ] {
            assert_eq!(displayed_popularity(raw), expected);
        }
    }

    #[test]
    fn popularity_color_uses_native_band_before_display_rounding() {
        assert_eq!(displayed_popularity(9_499), 95);
        assert_eq!(displayed_popularity(9_500), 95);
        assert_eq!(popularity_color(9_499), spawn_rate_color(36));
        assert_eq!(popularity_color(9_500), spawn_rate_color(40));
        assert_ne!(popularity_color(9_499), popularity_color(9_500));
        assert_eq!(popularity_color(4_999), spawn_rate_color(-1));
        assert_eq!(popularity_color(5_000), spawn_rate_color(10));
    }

    #[test]
    fn spawn_rate_colors_preserve_palette_anchors() {
        for (rate, rgb) in [
            (-8, [0xF2, 0x8C, 0x88]),
            (0, [0xF2, 0x8C, 0x88]),
            (10, [0xEC, 0xA3, 0x6A]),
            (20, [0xE3, 0xCB, 0x68]),
            (40, [0x76, 0xD2, 0x9A]),
        ] {
            assert_eq!(
                spawn_rate_color(rate),
                Color::from_rgb_u8(rgb[0], rgb[1], rgb[2])
            );
        }
        let positive_rates = [10, 12, 14, 16, 20, 24, 28, 32, 36, 40];
        for pair in positive_rates.windows(2) {
            assert_ne!(spawn_rate_color(pair[0]), spawn_rate_color(pair[1]));
        }
    }

    #[test]
    fn popularity_color_keeps_equal_rates_and_clamps_invalid_values() {
        let red = Color::from_rgb_u8(0xF2, 0x8C, 0x88);
        let green = Color::from_rgb_u8(0x76, 0xD2, 0x9A);
        for raw in [i32::MIN, -1, 0, 2_000, 2_500, 4_999] {
            assert_eq!(popularity_color(raw), red);
        }
        for raw in [9_500, 9_999, 10_000, 10_001, i32::MAX] {
            assert_eq!(popularity_color(raw), green);
        }
    }
}
