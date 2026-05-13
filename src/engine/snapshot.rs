use std::time::Instant;

use super::building::{aggregate_buildings, workshop_count, BuildingSnapshot};
use super::enums::{building_type, unit_type};
use super::game::{GameManagerSnapshot, PostGameStatisticsSnapshot};
use super::player::PlayerSnapshot;
use super::unit::{aggregate_units, count_unit, shooter_count, ShooterCountOptions, UnitSnapshot};
use super::unit::{
    camel_lancer_count, catapult_count, horse_archer_count, shield_count, skirmisher_count,
};

#[derive(Debug, Clone)]
pub struct GameSnapshot {
    pub game: GameManagerSnapshot,
    pub players: Vec<PlayerSnapshot>,
    pub units: Vec<UnitSnapshot>,
    pub buildings: Vec<BuildingSnapshot>,
    pub post_game_statistics: PostGameStatisticsSnapshot,
    pub read_time: Instant,
}

#[derive(Debug, Clone)]
pub struct PlayerOverlayStats {
    pub player_id: u16,
    pub gold: i32,
    pub wood: i32,
    pub stone: i32,
    pub food_stock: i32,
    pub food_ratio_effect: i32,
    pub tax_effect: i32,
    pub total_popularity_effect: i32,
    pub popularity: i32,
    pub population: i32,
    pub army: i32,
    pub buildings: i32,
    pub traps: i32,
    pub workshops: i32,
    pub ff: i32,
    pub ha: i32,
    pub skirmishers: i32,
    pub archers: i32,
    pub xbows: i32,
    pub arab_bows: i32,
    pub slingers: i32,
    pub camel_lancers: i32,
    pub knights: i32,
    pub shields: i32,
    pub catapults: i32,
    pub shooters: i32,
    pub army_killed: i32,
    pub army_lost: i32,
    pub extreme_power: i32,
    pub stat_gold_earned: i32,
    pub stat_food_produced: i32,
    pub stat_wood_produced: i32,
    pub stat_stone_produced: i32,
    pub stat_iron_produced: i32,
    pub stat_weapons_produced: i32,
}

pub fn overlay_rows(snapshot: &GameSnapshot) -> Vec<PlayerOverlayStats> {
    let unit_stats = aggregate_units(&snapshot.units);
    let building_stats = aggregate_buildings(&snapshot.buildings);
    let shooter_options = ShooterCountOptions::default();

    snapshot
        .players
        .iter()
        .map(|player| {
            let player_id = player.player_id as u16;
            let units = unit_stats.get(&player_id);
            let buildings = building_stats.get(&player_id);
            let post_game = snapshot
                .post_game_statistics
                .players
                .iter()
                .find(|stats| stats.player_id == player.player_id);
            let matrix_index = player.player_id.saturating_sub(1) as usize;
            let army_killed = snapshot
                .post_game_statistics
                .army_kills_matrix
                .iter()
                .find(|row| row.killer_player_id == player.player_id)
                .map(|row| row.killed_army_by_column.iter().take(8).sum::<u32>() as i32)
                .unwrap_or_default();
            let army_lost = snapshot
                .post_game_statistics
                .army_kills_matrix
                .iter()
                .map(|row| {
                    row.killed_army_by_column
                        .get(matrix_index)
                        .copied()
                        .unwrap_or_default()
                })
                .sum::<u32>() as i32;

            PlayerOverlayStats {
                player_id,
                gold: player.gold as i32,
                wood: player.wood as i32,
                stone: player.stone as i32,
                food_stock: player.food_stock() as i32,
                food_ratio_effect: player.food_ration_effect(),
                tax_effect: player.tax_effect(),
                total_popularity_effect: player.total_popularity_effect(),
                popularity: player.popularity,
                population: player.population_total,
                army: player.army_size,
                buildings: buildings
                    .map(|stats| stats.total_buildings as i32)
                    .unwrap_or_default(),
                traps: buildings
                    .and_then(|stats| stats.buildings_by_type.get(&building_type::KILLING_PIT))
                    .copied()
                    .unwrap_or_default() as i32,
                workshops: buildings.map(workshop_count).unwrap_or_default() as i32,
                ff: player.fear_effect,
                ha: units.map(horse_archer_count).unwrap_or_default() as i32,
                skirmishers: units.map(skirmisher_count).unwrap_or_default() as i32,
                archers: units
                    .map(|stats| count_unit(stats, unit_type::ARCHER))
                    .unwrap_or_default() as i32,
                xbows: units
                    .map(|stats| count_unit(stats, unit_type::XBOWMAN))
                    .unwrap_or_default() as i32,
                arab_bows: units
                    .map(|stats| count_unit(stats, unit_type::ARAB_BOW))
                    .unwrap_or_default() as i32,
                slingers: units
                    .map(|stats| count_unit(stats, unit_type::ARAB_SLINGER))
                    .unwrap_or_default() as i32,
                camel_lancers: units.map(camel_lancer_count).unwrap_or_default() as i32,
                knights: units
                    .map(|stats| count_unit(stats, unit_type::KNIGHT))
                    .unwrap_or_default() as i32,
                shields: units.map(shield_count).unwrap_or_default() as i32,
                catapults: units.map(catapult_count).unwrap_or_default() as i32,
                shooters: units
                    .map(|stats| shooter_count(stats, &shooter_options) as i32)
                    .unwrap_or_default(),
                army_killed,
                army_lost,
                extreme_power: player.extreme_power_bar as i32,
                stat_gold_earned: post_game
                    .map(|stats| stats.gold_earning as i32)
                    .unwrap_or_default(),
                stat_food_produced: post_game
                    .map(|stats| stats.food_score as i32)
                    .unwrap_or_default(),
                stat_wood_produced: post_game
                    .map(|stats| stats.wood_produced as i32)
                    .unwrap_or_default(),
                stat_stone_produced: post_game
                    .map(|stats| stats.total_produced_stone as i32)
                    .unwrap_or_default(),
                stat_iron_produced: post_game
                    .map(|stats| stats.iron_produced as i32)
                    .unwrap_or_default(),
                stat_weapons_produced: post_game
                    .map(|stats| stats.weapons_produced as i32)
                    .unwrap_or_default(),
            }
        })
        .collect()
}
