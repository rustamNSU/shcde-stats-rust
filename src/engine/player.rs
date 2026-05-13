use anyhow::Result;

use super::addresses::{player_offsets, MAX_PLAYERS, PLAYER_STRIDE, PLAYER_TABLE_ANCHOR};
use super::memory::{checked_offset, MemoryRead};

const FOOD_RATION_EFFECTS: [i32; 5] = [-8, -4, 0, 4, 8];
const TAX_EFFECTS: [i32; 12] = [7, 5, 3, 1, -2, -4, -6, -8, -12, -16, -20, -24];

#[derive(Debug, Clone, Default)]
pub struct PlayerSnapshot {
    pub slot: usize,
    pub player_id: u32,
    pub gold: u32,
    pub wood: u32,
    pub stone: u32,
    pub bread: u32,
    pub cheese: u32,
    pub meat: u32,
    pub apples: u32,
    pub food_types_eaten: u16,
    pub tax_type: u16,
    pub food_ratio: u16,
    pub inn_popularity_modifier_raw: i32,
    pub religion_popularity_modifier_raw: i32,
    pub fear_factor_popularity_modifier_raw: i32,
    pub tax_popularity_modifier_raw: i32,
    pub rations_popularity_modifier_raw: i32,
    pub crowding_popularity_modifier_raw: i32,
    pub inn_effect: i32,
    pub religion_effect: i32,
    pub fear_effect: i32,
    pub tax_effect: i32,
    pub food_effect: i32,
    pub crowding_effect: i32,
    pub population_total: i32,
    pub population_on_bonfire: i32,
    pub popularity: i32,
    pub current_population: i32,
    pub army_size: i32,
    /// Fear factor effect. Memory type is a 4-byte signed integer.
    pub troops_power: i32,
    pub extreme_power_bar: u32,
}

impl PlayerSnapshot {
    pub fn food_stock(&self) -> u32 {
        self.bread + self.cheese + self.meat + self.apples
    }

    pub fn food_ration_effect(&self) -> i32 {
        self.food_effect
    }

    pub fn tax_effect(&self) -> i32 {
        self.tax_effect
    }

    pub fn total_popularity_effect(&self) -> i32 {
        self.tax_effect
            + self.food_effect
            + self.crowding_effect
            + self.religion_effect
            + self.inn_effect
            + self.fear_effect
    }
}

pub struct PlayerTableReader<'a, R> {
    memory: &'a R,
    crusader_base: usize,
    max_players: usize,
}

impl<'a, R: MemoryRead> PlayerTableReader<'a, R> {
    pub fn new(memory: &'a R, crusader_base: usize) -> Self {
        Self {
            memory,
            crusader_base,
            max_players: MAX_PLAYERS,
        }
    }

    pub fn read_all(&self) -> Result<Vec<PlayerSnapshot>> {
        let mut players = Vec::with_capacity(self.max_players);

        for slot in 0..self.max_players {
            players.push(self.read_slot(slot)?);
        }

        Ok(players)
    }

    fn read_slot(&self, slot: usize) -> Result<PlayerSnapshot> {
        let row_base = self
            .crusader_base
            .checked_add(PLAYER_TABLE_ANCHOR)
            .and_then(|base| base.checked_add(slot * PLAYER_STRIDE))
            .ok_or_else(|| anyhow::anyhow!("player row address overflow for slot {slot}"))?;

        let read_u16 = |offset| self.memory.read_u16(checked_offset(row_base, offset)?);
        let read_u32 = |offset| self.memory.read_u32(checked_offset(row_base, offset)?);
        let read_i32 = |offset| self.memory.read_i32(checked_offset(row_base, offset)?);

        let food_ratio = read_u16(player_offsets::FOOD_RATIO)?;
        let food_types_eaten = read_u16(player_offsets::FOOD_TYPES_EATEN)?;
        let tax_type = read_u16(player_offsets::TAX_TYPE)?;
        let inn_popularity_modifier_raw = read_i32(player_offsets::INN_POPULARITY_MODIFIER)?;
        let religion_popularity_modifier_raw =
            read_i32(player_offsets::RELIGION_POPULARITY_MODIFIER)?;
        let fear_factor_popularity_modifier_raw =
            read_i32(player_offsets::FEAR_FACTOR_POPULARITY_MODIFIER)?;
        let tax_popularity_modifier_raw = read_i32(player_offsets::TAX_POPULARITY_MODIFIER)?;
        let rations_popularity_modifier_raw =
            read_i32(player_offsets::RATIONS_POPULARITY_MODIFIER)?;
        let crowding_popularity_modifier_raw =
            read_i32(player_offsets::CROWDING_POPULARITY_MODIFIER)?;

        Ok(PlayerSnapshot {
            slot,
            player_id: (slot + 1) as u32,
            gold: read_u32(player_offsets::GOLD)?,
            wood: u32::from(read_u16(player_offsets::WOOD)?),
            stone: u32::from(read_u16(player_offsets::STONE)?),
            bread: u32::from(read_u16(player_offsets::BREAD)?),
            cheese: u32::from(read_u16(player_offsets::CHEESE)?),
            meat: u32::from(read_u16(player_offsets::MEAT)?),
            apples: u32::from(read_u16(player_offsets::APPLES)?),
            food_types_eaten,
            tax_type,
            food_ratio,
            inn_popularity_modifier_raw,
            religion_popularity_modifier_raw,
            fear_factor_popularity_modifier_raw,
            tax_popularity_modifier_raw,
            rations_popularity_modifier_raw,
            crowding_popularity_modifier_raw,
            inn_effect: popularity_modifier_effect(inn_popularity_modifier_raw),
            religion_effect: popularity_modifier_effect(religion_popularity_modifier_raw),
            fear_effect: popularity_modifier_effect(fear_factor_popularity_modifier_raw),
            tax_effect: popularity_modifier_effect(tax_popularity_modifier_raw),
            food_effect: popularity_modifier_effect(rations_popularity_modifier_raw),
            crowding_effect: popularity_modifier_effect(crowding_popularity_modifier_raw),
            population_total: read_i32(player_offsets::POPULATION_TOTAL)?,
            population_on_bonfire: read_i32(player_offsets::POPULATION_ON_BONFIRE)?,
            popularity: read_i32(player_offsets::POPULARITY)?,
            current_population: read_i32(player_offsets::CURRENT_POPULATION)?,
            army_size: read_i32(player_offsets::ARMY_SIZE)?,
            troops_power: read_i32(player_offsets::TROOPS_POWER)?,
            extreme_power_bar: read_u32(player_offsets::EXTREME_POWER_BAR)?,
        })
    }
}

pub fn signed_effect_text(value: i32) -> String {
    if value > 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}

pub fn popularity_modifier_effect(raw: i32) -> i32 {
    raw / 25
}

pub fn food_ration_effect(food_ratio: u16, food_types_eaten: u16) -> i32 {
    if food_types_eaten == 0 {
        return -8;
    }

    let base = FOOD_RATION_EFFECTS
        .get(food_ratio as usize)
        .copied()
        .unwrap_or(0);
    base + food_types_eaten as i32 - 1
}

pub fn tax_effect(tax_type: u16) -> i32 {
    TAX_EFFECTS.get(tax_type as usize).copied().unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{food_ration_effect, popularity_modifier_effect, signed_effect_text, tax_effect};

    #[test]
    fn food_ration_no_food_overrides_ratio() {
        assert_eq!(food_ration_effect(0, 0), -8);
        assert_eq!(food_ration_effect(4, 0), -8);
    }

    #[test]
    fn food_ration_adds_food_variety() {
        assert_eq!(food_ration_effect(4, 1), 8);
        assert_eq!(food_ration_effect(4, 4), 11);
    }

    #[test]
    fn tax_effect_uses_known_table() {
        assert_eq!(tax_effect(0), 7);
        assert_eq!(tax_effect(3), 1);
        assert_eq!(tax_effect(7), -8);
    }

    #[test]
    fn signed_effect_text_adds_plus_for_positive_values() {
        assert_eq!(signed_effect_text(7), "+7");
        assert_eq!(signed_effect_text(0), "0");
        assert_eq!(signed_effect_text(-8), "-8");
    }

    #[test]
    fn popularity_modifier_effect_scales_raw_values() {
        assert_eq!(popularity_modifier_effect(100), 4);
        assert_eq!(popularity_modifier_effect(75), 3);
        assert_eq!(popularity_modifier_effect(-125), -5);
        assert_eq!(popularity_modifier_effect(-200), -8);
        assert_eq!(popularity_modifier_effect(200), 8);
        assert_eq!(popularity_modifier_effect(0), 0);
    }
}
