use std::collections::HashMap;

use anyhow::Result;

use super::addresses::{unit_offsets, DEFAULT_MAX_UNITS, UNIT_STRIDE, UNIT_TABLE_ANCHOR};
use super::enums::{is_population_unit, unit_type};
use super::memory::{checked_offset, MemoryRead};

#[derive(Debug, Clone, Default)]
pub struct UnitSnapshot {
    pub index: usize,
    pub type_id: u16,
    pub global_id: u32,
    pub player_id: u16,
    pub current_hp: u32,
    pub max_hp: u32,
}

pub struct UnitTableReader<'a, R> {
    memory: &'a R,
    crusader_base: usize,
    max_units: usize,
}

impl<'a, R: MemoryRead> UnitTableReader<'a, R> {
    pub fn new(memory: &'a R, crusader_base: usize, max_units: usize) -> Self {
        Self {
            memory,
            crusader_base,
            max_units,
        }
    }

    pub fn read_all(&self) -> Result<Vec<UnitSnapshot>> {
        let mut units = Vec::new();
        let max_units = self.max_units.min(DEFAULT_MAX_UNITS);

        for index in 0..max_units {
            let unit = self.read_slot(index)?;
            if unit.is_valid() {
                units.push(unit);
            }
        }

        Ok(units)
    }

    fn read_slot(&self, index: usize) -> Result<UnitSnapshot> {
        let row_base = self
            .crusader_base
            .checked_add(UNIT_TABLE_ANCHOR)
            .and_then(|base| base.checked_add(index * UNIT_STRIDE))
            .ok_or_else(|| anyhow::anyhow!("unit row address overflow for index {index}"))?;

        let read_u16 = |offset| self.memory.read_u16(checked_offset(row_base, offset)?);
        let read_u32 = |offset| self.memory.read_u32(checked_offset(row_base, offset)?);

        Ok(UnitSnapshot {
            index,
            type_id: read_u16(unit_offsets::TYPE)?,
            global_id: read_u32(unit_offsets::GLOBAL_ID)?,
            player_id: read_u16(unit_offsets::PLAYER_ID)?,
            current_hp: read_u32(unit_offsets::CURRENT_HP)?,
            max_hp: read_u32(unit_offsets::MAX_HP)?,
        })
    }
}

impl UnitSnapshot {
    pub fn is_valid(&self) -> bool {
        self.type_id > 0 || self.current_hp > 0 || self.player_id > 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlayerUnitStats {
    pub player_id: u16,
    pub total_units: usize,
    pub population_units: usize,
    pub army_units: usize,
    pub units_by_type: HashMap<u16, usize>,
}

#[derive(Debug, Clone)]
pub struct ShooterCountOptions {
    pub count_ha: bool,
    pub count_skirmishers: bool,
    pub count_archers: bool,
    pub count_xbows: bool,
    pub count_arab_bows: bool,
    pub count_slingers: bool,
}

impl Default for ShooterCountOptions {
    fn default() -> Self {
        Self {
            count_ha: true,
            count_skirmishers: false,
            count_archers: false,
            count_xbows: false,
            count_arab_bows: false,
            count_slingers: false,
        }
    }
}

pub fn aggregate_units(units: &[UnitSnapshot]) -> HashMap<u16, PlayerUnitStats> {
    let mut stats = HashMap::new();

    for unit in units.iter().filter(|unit| unit.is_valid()) {
        let row = stats
            .entry(unit.player_id)
            .or_insert_with(|| PlayerUnitStats {
                player_id: unit.player_id,
                ..PlayerUnitStats::default()
            });

        row.total_units += 1;
        *row.units_by_type.entry(unit.type_id).or_default() += 1;

        if is_population_unit(unit.type_id) {
            row.population_units += 1;
        } else {
            row.army_units += 1;
        }
    }

    stats
}

pub fn count_unit(stats: &PlayerUnitStats, unit_type_id: u16) -> usize {
    stats.units_by_type.get(&unit_type_id).copied().unwrap_or(0)
}

pub fn shooter_count(stats: &PlayerUnitStats, options: &ShooterCountOptions) -> usize {
    let mut total = 0;

    if options.count_ha {
        total += count_unit(stats, unit_type::ARAB_HORSEMAN);
    }
    if options.count_skirmishers {
        total += count_unit(stats, unit_type::BEDOUIN_SKIRMISHER);
    }
    if options.count_archers {
        total += count_unit(stats, unit_type::ARCHER);
    }
    if options.count_xbows {
        total += count_unit(stats, unit_type::XBOWMAN);
    }
    if options.count_arab_bows {
        total += count_unit(stats, unit_type::ARAB_BOW);
    }
    if options.count_slingers {
        total += count_unit(stats, unit_type::ARAB_SLINGER);
    }

    total
}

pub fn horse_archer_count(stats: &PlayerUnitStats) -> usize {
    count_unit(stats, unit_type::ARAB_HORSEMAN)
}

pub fn skirmisher_count(stats: &PlayerUnitStats) -> usize {
    count_unit(stats, unit_type::BEDOUIN_SKIRMISHER)
}

pub fn camel_lancer_count(stats: &PlayerUnitStats) -> usize {
    count_unit(stats, unit_type::BEDOUIN_CAMEL_LANCER)
}

pub fn shield_count(stats: &PlayerUnitStats) -> usize {
    count_unit(stats, unit_type::PORTABLE_SHIELD)
}

pub fn catapult_count(stats: &PlayerUnitStats) -> usize {
    count_unit(stats, unit_type::CATAPULT)
}

#[cfg(test)]
mod tests {
    use super::{aggregate_units, shooter_count, ShooterCountOptions, UnitSnapshot};
    use crate::engine::enums::unit_type;

    #[test]
    fn aggregate_units_splits_population_and_army() {
        let stats = aggregate_units(&[
            UnitSnapshot {
                player_id: 1,
                type_id: unit_type::WOODCUTTER,
                ..UnitSnapshot::default()
            },
            UnitSnapshot {
                player_id: 1,
                type_id: unit_type::ARAB_HORSEMAN,
                ..UnitSnapshot::default()
            },
        ]);

        let player = stats.get(&1).expect("player stats");
        assert_eq!(player.population_units, 1);
        assert_eq!(player.army_units, 1);
    }

    #[test]
    fn default_shooter_count_is_ha_only() {
        let stats = aggregate_units(&[
            UnitSnapshot {
                player_id: 1,
                type_id: unit_type::ARAB_HORSEMAN,
                ..UnitSnapshot::default()
            },
            UnitSnapshot {
                player_id: 1,
                type_id: unit_type::BEDOUIN_SKIRMISHER,
                ..UnitSnapshot::default()
            },
            UnitSnapshot {
                player_id: 1,
                type_id: unit_type::ARCHER,
                ..UnitSnapshot::default()
            },
        ]);

        assert_eq!(
            shooter_count(
                stats.get(&1).expect("player stats"),
                &ShooterCountOptions::default()
            ),
            1
        );
    }
}
