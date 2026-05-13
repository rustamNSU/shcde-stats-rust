use std::collections::HashMap;

use anyhow::Result;

use super::addresses::{
    building_offsets, BUILDING_STRIDE, BUILDING_STRUCT_START_OFFSET, BUILDING_TABLE_ANCHOR,
    DEFAULT_MAX_BUILDINGS, FIRST_BUILDING_INDEX,
};
use super::enums::building_type;
use super::memory::{checked_offset, MemoryRead};

#[derive(Debug, Clone, Default)]
pub struct BuildingSnapshot {
    pub index: usize,
    pub type_id: u16,
    pub owner_player_id: u16,
    pub global_id: u32,
    pub current_hp: i16,
    pub max_hp: u16,
}

pub struct BuildingTableReader<'a, R> {
    memory: &'a R,
    crusader_base: usize,
    max_buildings: usize,
}

impl<'a, R: MemoryRead> BuildingTableReader<'a, R> {
    pub fn new(memory: &'a R, crusader_base: usize, max_buildings: usize) -> Self {
        Self {
            memory,
            crusader_base,
            max_buildings,
        }
    }

    pub fn read_all(&self) -> Result<Vec<BuildingSnapshot>> {
        let mut buildings = Vec::new();
        let max_buildings = self.max_buildings.min(DEFAULT_MAX_BUILDINGS);

        for index in FIRST_BUILDING_INDEX..=max_buildings {
            let building = self.read_slot(index)?;
            if building.is_valid() {
                buildings.push(building);
            }
        }

        Ok(buildings)
    }

    fn read_slot(&self, index: usize) -> Result<BuildingSnapshot> {
        let row_base = self
            .crusader_base
            .checked_add(BUILDING_TABLE_ANCHOR)
            .and_then(|base| base.checked_add(index * BUILDING_STRIDE))
            .and_then(|base| base.checked_add(BUILDING_STRUCT_START_OFFSET))
            .ok_or_else(|| anyhow::anyhow!("building row address overflow for index {index}"))?;

        let read_u16 = |offset| self.memory.read_u16(checked_offset(row_base, offset)?);
        let read_i16 = |offset| self.memory.read_i16(checked_offset(row_base, offset)?);
        let read_u32 = |offset| self.memory.read_u32(checked_offset(row_base, offset)?);

        Ok(BuildingSnapshot {
            index,
            type_id: read_u16(building_offsets::TYPE)?,
            owner_player_id: read_u16(building_offsets::OWNER_PLAYER_ID)?,
            global_id: read_u32(building_offsets::GLOBAL_ID)?,
            current_hp: read_i16(building_offsets::CURRENT_HP)?,
            max_hp: read_u16(building_offsets::MAX_HP)?,
        })
    }
}

impl BuildingSnapshot {
    pub fn is_valid(&self) -> bool {
        self.type_id > 0
            || self.max_hp > 0
            || self.current_hp > 0
            || self.owner_player_id > 0
            || self.global_id > 0
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlayerBuildingStats {
    pub player_id: u16,
    pub total_buildings: usize,
    pub buildings_by_type: HashMap<u16, usize>,
}

pub fn aggregate_buildings(buildings: &[BuildingSnapshot]) -> HashMap<u16, PlayerBuildingStats> {
    let mut stats = HashMap::new();

    for building in buildings.iter().filter(|building| building.is_valid()) {
        let row = stats
            .entry(building.owner_player_id)
            .or_insert_with(|| PlayerBuildingStats {
                player_id: building.owner_player_id,
                ..PlayerBuildingStats::default()
            });

        row.total_buildings += 1;
        *row.buildings_by_type.entry(building.type_id).or_default() += 1;
    }

    stats
}

pub fn workshop_count(stats: &PlayerBuildingStats) -> usize {
    [
        building_type::FLETCHERS_WORKSHOP,
        building_type::POLETURNERS_WORKSHOP,
        building_type::ARMOURERS_WORKSHOP,
        building_type::BLACKSMITHS_WORKSHOP,
        building_type::TANNERS_WORKSHOP,
    ]
    .into_iter()
    .map(|type_id| stats.buildings_by_type.get(&type_id).copied().unwrap_or(0))
    .sum()
}
