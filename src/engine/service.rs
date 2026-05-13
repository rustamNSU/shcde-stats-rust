use std::time::Instant;

use super::addresses::UI_PLAYER_ID;
use super::building::{BuildingSnapshot, BuildingTableReader};
use super::game::{GameManagerReader, GameManagerSnapshot, PostGameStatisticsSnapshot};
use super::memory::MemoryWrite;
use super::player::{PlayerSnapshot, PlayerTableReader};
use super::process::{attach_to_known_game_process, GameProcess};
use super::snapshot::GameSnapshot;
use super::unit::{UnitSnapshot, UnitTableReader};

#[derive(Debug, Clone)]
pub struct EngineOptions {
    pub update_interval_ms: u64,
    pub max_units: usize,
    pub max_buildings: usize,
}

impl Default for EngineOptions {
    fn default() -> Self {
        Self {
            update_interval_ms: 50,
            max_units: 9999,
            max_buildings: 3999,
        }
    }
}

pub struct LiveStatsSource {
    process: Option<GameProcess>,
    last_error: Option<String>,
    heavy_refresh_interval_ms: u64,
    last_heavy_refresh: Option<Instant>,
    cached_units: Vec<UnitSnapshot>,
    cached_buildings: Vec<BuildingSnapshot>,
    cached_post_game_statistics: PostGameStatisticsSnapshot,
}

impl LiveStatsSource {
    pub fn new() -> Self {
        Self {
            process: None,
            last_error: None,
            heavy_refresh_interval_ms: 200,
            last_heavy_refresh: None,
            cached_units: Vec::new(),
            cached_buildings: Vec::new(),
            cached_post_game_statistics: PostGameStatisticsSnapshot::default(),
        }
    }

    pub fn set_heavy_refresh_interval_ms(&mut self, interval_ms: u64) {
        self.heavy_refresh_interval_ms = interval_ms.clamp(100, 2_000);
    }

    pub fn snapshot(&mut self) -> GameSnapshot {
        match self.read_snapshot() {
            Ok(snapshot) => {
                self.last_error = None;
                snapshot
            }
            Err(err) => {
                self.last_error = Some(format!("{err:#}"));
                empty_snapshot()
            }
        }
    }

    pub fn status_text(&self) -> String {
        match (&self.process, &self.last_error) {
            (Some(process), None) => format!(
                "Attached to SHC:DE pid {}. CrusaderDE.dll base {:#x}. Read-only.",
                process.process_id(),
                process.crusader_base()
            ),
            (Some(_), Some(error)) => format!("Attached, read failed: {error}"),
            (None, Some(error)) => format!("Not attached: {error}"),
            (None, None) => "Not attached yet.".to_string(),
        }
    }

    pub fn write_ui_player_id(&mut self, player_id: u32) -> anyhow::Result<()> {
        if self.process.is_none() {
            self.process = Some(attach_to_known_game_process()?);
        }

        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        process.write_u32(process.module_addr(UI_PLAYER_ID), player_id.clamp(1, 8))
    }

    fn read_snapshot(&mut self) -> anyhow::Result<GameSnapshot> {
        if self.process.is_none() {
            self.process = Some(attach_to_known_game_process()?);
        }

        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        let game_reader = GameManagerReader::new(process, process.crusader_base());
        let game = game_reader.read()?;
        let players = PlayerTableReader::new(process, process.crusader_base()).read_all()?;

        let should_refresh_heavy = self
            .last_heavy_refresh
            .map(|last| last.elapsed().as_millis() >= u128::from(self.heavy_refresh_interval_ms))
            .unwrap_or(true);

        if should_refresh_heavy {
            self.cached_units =
                UnitTableReader::new(process, process.crusader_base(), 9999).read_all()?;
            self.cached_buildings =
                BuildingTableReader::new(process, process.crusader_base(), 3999).read_all()?;
            self.cached_post_game_statistics = game_reader.read_post_game_statistics()?;
            self.last_heavy_refresh = Some(Instant::now());
        }

        Ok(GameSnapshot {
            game,
            players,
            units: self.cached_units.clone(),
            buildings: self.cached_buildings.clone(),
            post_game_statistics: self.cached_post_game_statistics.clone(),
            read_time: Instant::now(),
        })
    }
}

pub fn empty_snapshot() -> GameSnapshot {
    GameSnapshot {
        game: GameManagerSnapshot::default(),
        players: (1..=8).map(zero_player).collect(),
        units: Vec::new(),
        buildings: Vec::new(),
        post_game_statistics: PostGameStatisticsSnapshot::default(),
        read_time: Instant::now(),
    }
}

fn zero_player(player_id: u32) -> PlayerSnapshot {
    PlayerSnapshot {
        slot: (player_id - 1) as usize,
        player_id,
        ..PlayerSnapshot::default()
    }
}
