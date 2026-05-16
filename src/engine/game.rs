use anyhow::Result;

use super::addresses::{
    post_game_offsets, BUILD_ANYWHERE, EXTREME_POWER_BAR_ANY_MODE, EXTREME_POWER_BAR_PLAYER1,
    GAME_MANAGER_BASE, KILL_MATRIX_BASE_OFFSET, KILL_MATRIX_COLUMNS, KILL_MATRIX_COLUMN_STRIDE,
    KILL_MATRIX_ROW_STRIDE, LAST_SELECTED_EXTREME_POWER, MAX_PLAYERS, MONTH_FIELD,
    MONTH_PART_FIELD, MONTH_UNTIL_DEFEAT, PLACEMENT_STATE_EVEN, PLACEMENT_TYPE, PLACEMENT_X,
    PLACEMENT_Y, SANDS_TIMER, SCENARIO_MODE, SELECTED_EXTREME_POWER_UI_FLAG, STATS_CURRENT_MONTH,
    STATS_CURRENT_YEAR, STATS_START_MONTH, STATS_START_YEAR, TICKS_FIELD, UI_PLAYER_ID, YEAR_FIELD,
};
use super::memory::MemoryRead;

#[derive(Debug, Clone, Default)]
pub struct GameManagerSnapshot {
    pub sands_timer: u32,
    pub scenario_mode: u32,
    pub month_part: u32,
    pub month: u32,
    pub month_until_defeat: u32,
    pub year: u32,
    pub ticks: u32,
    pub extreme_power_bar_player1: u32,
    pub extreme_power_bar_any_mode: u32,
    pub build_anywhere: u32,
    pub placement_state_even: u32,
    pub placement_type: u32,
    pub placement_x: u32,
    pub placement_y: u32,
    pub selected_extreme_power_ui_flag: u32,
    pub last_selected_extreme_power: u32,
    pub ui_player_id: u32,
    pub stats_start_year: u32,
    pub stats_start_month: u32,
    pub stats_current_year: u32,
    pub stats_current_month: u32,
}

impl GameManagerSnapshot {
    pub fn is_plausible(&self) -> bool {
        self.year > 0 && self.month < 12 && self.month_part < 4
    }
}

#[derive(Debug, Clone, Default)]
pub struct PostGamePlayerStats {
    pub player_id: u32,
    pub gold_earning: u32,
    pub max_pop_in_game: u16,
    pub ticks_to_death_alive_time: u32,
    pub food_score: u32,
    pub iron_produced: u32,
    pub total_produced_stone: u32,
    pub destroyed_buildings: u32,
    pub wood_produced: u32,
    pub weapons_produced: u32,
    pub lost_buildings: u32,
    pub army_recruited: u32,
}

#[derive(Debug, Clone, Default)]
pub struct KillMatrixRow {
    pub killer_player_id: u32,
    pub killed_army_by_column: [u32; KILL_MATRIX_COLUMNS],
}

#[derive(Debug, Clone, Default)]
pub struct PostGameStatisticsSnapshot {
    pub players: Vec<PostGamePlayerStats>,
    pub army_kills_matrix: Vec<KillMatrixRow>,
}

pub struct GameManagerReader<'a, R> {
    memory: &'a R,
    crusader_base: usize,
}

impl<'a, R: MemoryRead> GameManagerReader<'a, R> {
    pub fn new(memory: &'a R, crusader_base: usize) -> Self {
        Self {
            memory,
            crusader_base,
        }
    }

    pub fn read(&self) -> Result<GameManagerSnapshot> {
        let manager_base = self.crusader_base + GAME_MANAGER_BASE;

        Ok(GameManagerSnapshot {
            sands_timer: self.read_absolute(SANDS_TIMER)?,
            scenario_mode: self.read_absolute(SCENARIO_MODE)?,
            month_part: self.memory.read_u32(manager_base + MONTH_PART_FIELD)?,
            month: self.memory.read_u32(manager_base + MONTH_FIELD)?,
            month_until_defeat: self.read_absolute(MONTH_UNTIL_DEFEAT)?,
            year: self.memory.read_u32(manager_base + YEAR_FIELD)?,
            ticks: self.memory.read_u32(manager_base + TICKS_FIELD)?,
            extreme_power_bar_player1: self.read_absolute(EXTREME_POWER_BAR_PLAYER1)?,
            extreme_power_bar_any_mode: self.read_absolute(EXTREME_POWER_BAR_ANY_MODE)?,
            build_anywhere: self.read_absolute(BUILD_ANYWHERE)?,
            placement_state_even: self.read_absolute(PLACEMENT_STATE_EVEN)?,
            placement_type: self.read_absolute(PLACEMENT_TYPE)?,
            placement_x: self.read_absolute(PLACEMENT_X)?,
            placement_y: self.read_absolute(PLACEMENT_Y)?,
            selected_extreme_power_ui_flag: self.read_absolute(SELECTED_EXTREME_POWER_UI_FLAG)?,
            last_selected_extreme_power: self.read_absolute(LAST_SELECTED_EXTREME_POWER)?,
            ui_player_id: self.read_absolute(UI_PLAYER_ID)?,
            stats_start_year: self.read_absolute(STATS_START_YEAR)?,
            stats_start_month: self.read_absolute(STATS_START_MONTH)?,
            stats_current_year: self.read_absolute(STATS_CURRENT_YEAR)?,
            stats_current_month: self.read_absolute(STATS_CURRENT_MONTH)?,
        })
    }

    pub fn read_post_game_statistics(&self) -> Result<PostGameStatisticsSnapshot> {
        let mut players = Vec::with_capacity(MAX_PLAYERS);
        for player_id in 1..=MAX_PLAYERS {
            players.push(self.read_post_game_player_stats(player_id as u32)?);
        }

        let mut army_kills_matrix = Vec::with_capacity(MAX_PLAYERS);
        for killer_player_id in 1..=MAX_PLAYERS {
            let mut killed_army_by_column = [0u32; KILL_MATRIX_COLUMNS];

            for (column, value) in killed_army_by_column.iter_mut().enumerate() {
                let offset = KILL_MATRIX_BASE_OFFSET
                    + (killer_player_id - 1) * KILL_MATRIX_ROW_STRIDE
                    + column * KILL_MATRIX_COLUMN_STRIDE;
                *value = self.read_absolute(offset)?;
            }

            army_kills_matrix.push(KillMatrixRow {
                killer_player_id: killer_player_id as u32,
                killed_army_by_column,
            });
        }

        Ok(PostGameStatisticsSnapshot {
            players,
            army_kills_matrix,
        })
    }

    fn read_post_game_player_stats(&self, player_id: u32) -> Result<PostGamePlayerStats> {
        let dword_index = player_id as usize * post_game_offsets::DWORD_STRIDE;
        let word_index = player_id as usize * post_game_offsets::WORD_STRIDE;

        Ok(PostGamePlayerStats {
            player_id,
            gold_earning: self.read_absolute(post_game_offsets::GOLD_EARNING + dword_index)?,
            max_pop_in_game: self
                .read_absolute_u16(post_game_offsets::MAX_POP_IN_GAME + word_index)?,
            ticks_to_death_alive_time: self
                .read_absolute(post_game_offsets::TICKS_TO_DEATH_ALIVE_TIME + dword_index)?,
            food_score: self.read_absolute(post_game_offsets::FOOD_SCORE + dword_index)?,
            iron_produced: self.read_absolute(post_game_offsets::IRON_PRODUCED + dword_index)?,
            total_produced_stone: self
                .read_absolute(post_game_offsets::TOTAL_PRODUCED_STONE + dword_index)?,
            destroyed_buildings: self
                .read_absolute(post_game_offsets::DESTROYED_BUILDINGS + dword_index)?,
            wood_produced: self.read_absolute(post_game_offsets::WOOD_PRODUCED + dword_index)?,
            weapons_produced: self
                .read_absolute(post_game_offsets::WEAPONS_PRODUCED + dword_index)?,
            lost_buildings: self.read_absolute(post_game_offsets::LOST_BUILDINGS + dword_index)?,
            army_recruited: self.read_absolute(post_game_offsets::ARMY_RECRUITED + dword_index)?,
        })
    }

    fn read_absolute(&self, offset: usize) -> Result<u32> {
        self.memory.read_u32(self.crusader_base + offset)
    }

    fn read_absolute_u16(&self, offset: usize) -> Result<u16> {
        self.memory.read_u16(self.crusader_base + offset)
    }
}
