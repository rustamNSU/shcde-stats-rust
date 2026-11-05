use std::time::{Duration, Instant};

use super::addresses::{SCENARIO_MODE, UI_PLAYER_ID};
use super::building::{BuildingSnapshot, BuildingTableReader};
use super::game::{GameManagerReader, GameManagerSnapshot, PostGameStatisticsSnapshot};
use super::history_session::{HistorySession, NativeHistoryReader};
use super::memory::MemoryWrite;
use super::multiplayer::{MultiplayerPlayers, Runtime};
use super::player::{PlayerSnapshot, PlayerTableReader};
use super::process::{attach_to_known_game_process, GameProcess};
use super::snapshot::GameSnapshot;
use super::unit::{UnitSnapshot, UnitTableReader};

#[derive(Debug, Clone)]
pub struct HistorySample {
    pub snapshot: GameSnapshot,
    pub session: HistorySession,
}

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
    multiplayer_runtime: Option<Runtime>,
    last_runtime_discovery: Option<Instant>,
    last_game_tick: Option<u32>,
    last_history_snapshot: Option<GameSnapshot>,
    last_history_session: Option<HistorySession>,
    history_process_exited: bool,
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
            multiplayer_runtime: None,
            last_runtime_discovery: None,
            last_game_tick: None,
            last_history_snapshot: None,
            last_history_session: None,
            history_process_exited: false,
        }
    }

    pub fn set_heavy_refresh_interval_ms(&mut self, interval_ms: u64) {
        self.heavy_refresh_interval_ms = interval_ms.clamp(100, 2_000);
    }

    pub fn reattach(&mut self) {
        self.process = None;
        self.last_error = None;
        self.last_heavy_refresh = None;
        self.cached_units.clear();
        self.cached_buildings.clear();
        self.cached_post_game_statistics = PostGameStatisticsSnapshot::default();
        self.multiplayer_runtime = None;
        self.last_runtime_discovery = None;
        self.last_game_tick = None;
        self.last_history_snapshot = None;
        self.last_history_session = None;
    }

    /// Unknown read/attach failures do not imply that the game exited.
    pub fn history_process_running(&self) -> bool {
        !self.history_process_exited
    }

    fn ensure_connected(&mut self) -> anyhow::Result<()> {
        if let Some(process) = &self.process {
            if !process.is_alive()? {
                self.history_process_exited = true;
                self.reattach();
            }
        }
        if self.process.is_none() {
            self.process = Some(attach_to_known_game_process()?);
            self.history_process_exited = false;
        }
        Ok(())
    }

    fn notice_process_exit(&mut self) {
        if self
            .process
            .as_ref()
            .is_some_and(|process| matches!(process.is_alive(), Ok(false)))
        {
            self.history_process_exited = true;
            self.reattach();
        }
    }

    pub(crate) fn managed_history_session(&mut self) -> anyhow::Result<HistorySession> {
        self.ensure_connected()?;
        let result = (|| {
            let process = self
                .process
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
            if self.multiplayer_runtime.as_ref().is_none_or(|runtime| {
                !runtime.has_terminal_metadata()
                    && self
                        .last_runtime_discovery
                        .is_none_or(|last| last.elapsed() >= Duration::from_secs(5))
            }) {
                let (base, file) = process.mono_runtime()?;
                self.multiplayer_runtime = Some(Runtime::locate(process, base, &file)?);
                self.last_runtime_discovery = Some(Instant::now());
            }
            let mut session = self
                .multiplayer_runtime
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Multiplayer history reader unavailable"))?
                .history_session(process)?;
            session.process_id = process.process_id();
            Ok(session)
        })();
        if result.is_err() {
            self.multiplayer_runtime = None;
            self.notice_process_exit();
        }
        result
    }

    /// Unlike the display snapshot, a failed history read never produces zero statistics.
    pub fn history_sample(&mut self) -> anyhow::Result<HistorySample> {
        let result = self.read_history_sample();
        if result.is_err() {
            self.notice_process_exit();
        }
        result
    }

    fn read_history_sample(&mut self) -> anyhow::Result<HistorySample> {
        let before = self.managed_history_session()?;
        if let Some(sample) = self.terminal_history_sample(&before)? {
            return Ok(sample);
        }
        let active_world =
            before.multiplayer_active && before.director_multiplayer && before.world_loaded;
        if !active_world {
            let mut session = before;
            session.world_loaded = false;
            if !session.resyncing && (!session.multiplayer_active || !session.director_multiplayer)
            {
                self.last_history_session = None;
            }
            // The end screen can discard its roster before native data is readable.
            // Retain the actual last sample, including its original read timestamp.
            let snapshot = match &self.last_history_snapshot {
                Some(snapshot) => snapshot.clone(),
                None => self.read_snapshot()?,
            };
            return Ok(HistorySample { snapshot, session });
        }
        let snapshot = self.read_snapshot()?;
        let mut session = self.managed_history_session()?;
        anyhow::ensure!(
            before == session,
            "Multiplayer session changed during statistics read"
        );
        anyhow::ensure!(
            snapshot.game.is_plausible(),
            "The multiplayer world is not ready"
        );
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        NativeHistoryReader::new(process, process.crusader_base()).enrich(&mut session)?;
        anyhow::ensure!(
            before == self.managed_history_session()?,
            "Multiplayer session changed during setup read"
        );
        self.last_history_snapshot = Some(snapshot.clone());
        if confirmed_history_context(&session) {
            self.last_history_session = Some(session.clone());
        }
        Ok(HistorySample { snapshot, session })
    }

    fn terminal_history_sample(
        &mut self,
        before: &HistorySession,
    ) -> anyhow::Result<Option<HistorySample>> {
        let (Some(previous), Some(last_snapshot)) =
            (&self.last_history_session, &self.last_history_snapshot)
        else {
            return Ok(None);
        };
        if before.resyncing
            || previous.process_id != before.process_id
            || (!previous.completed && last_snapshot.read_time.elapsed() > Duration::from_secs(30))
            || before
                .game_token
                .as_ref()
                .is_some_and(|token| Some(token) != previous.game_token.as_ref())
        {
            return Ok(None);
        }
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        let runtime = self
            .multiplayer_runtime
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Multiplayer history reader unavailable"))?;
        if !runtime.mission_over_visible(process)? {
            return Ok(None);
        }
        if previous.completed {
            return Ok(Some(HistorySample {
                snapshot: last_snapshot.clone(),
                session: previous.clone(),
            }));
        }
        let previous = previous.clone();
        let last_tick = last_snapshot.game.ticks;
        self.last_heavy_refresh = None;
        let snapshot = self.read_snapshot()?;
        anyhow::ensure!(
            snapshot.game.is_plausible() && snapshot.game.ticks >= last_tick,
            "The ending multiplayer world changed"
        );
        let mut session = previous.clone();
        session.multiplayer_active = before.multiplayer_active;
        session.director_multiplayer = before.director_multiplayer;
        session.world_loaded = false;
        session.completed = true;
        session.resyncing = false;
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        NativeHistoryReader::new(process, process.crusader_base()).enrich(&mut session)?;
        anyhow::ensure!(
            session.map_name == previous.map_name
                && session.map_size == previous.map_size
                && session.game_type == previous.game_type,
            "The ending multiplayer map changed"
        );
        anyhow::ensure!(
            &self.managed_history_session()? == before,
            "Multiplayer lifecycle changed during the final read"
        );
        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        let runtime = self
            .multiplayer_runtime
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Multiplayer history reader unavailable"))?;
        anyhow::ensure!(
            runtime.mission_over_visible(process)?,
            "Mission-over screen changed during the final read"
        );
        self.last_history_snapshot = Some(snapshot.clone());
        self.last_history_session = Some(session.clone());
        Ok(Some(HistorySample { snapshot, session }))
    }

    /// Used by the appearance import and admin identity view; never changes manual choices.
    pub fn multiplayer_players(&mut self) -> anyhow::Result<MultiplayerPlayers> {
        self.ensure_connected()?;
        let result = (|| {
            let process = self
                .process
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
            if self.multiplayer_runtime.is_none() {
                let (base, file) = process.mono_runtime()?;
                self.multiplayer_runtime = Some(Runtime::locate(process, base, &file)?);
            }
            self.multiplayer_runtime
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Multiplayer name reader unavailable"))?
                .players(process)
        })();
        if result.is_err() {
            self.multiplayer_runtime = None;
            self.notice_process_exit();
        }
        result
    }

    pub fn snapshot(&mut self) -> GameSnapshot {
        match self.read_snapshot() {
            Ok(snapshot) => {
                self.last_error = None;
                snapshot
            }
            Err(err) => {
                self.last_error = Some(format!("{err:#}"));
                self.notice_process_exit();
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

    pub fn write_scenario_mode(&mut self, enabled: bool) -> anyhow::Result<()> {
        if self.process.is_none() {
            self.process = Some(attach_to_known_game_process()?);
        }

        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        process.write_u32(process.module_addr(SCENARIO_MODE), u32::from(enabled))
    }

    fn read_snapshot(&mut self) -> anyhow::Result<GameSnapshot> {
        self.ensure_connected()?;

        let process = self
            .process
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("process attach did not produce a handle"))?;
        let game_reader = GameManagerReader::new(process, process.crusader_base());
        let game = game_reader.read()?;
        let players = PlayerTableReader::new(process, process.crusader_base()).read_all()?;

        if self
            .last_game_tick
            .is_some_and(|previous| game.ticks < previous)
        {
            self.last_heavy_refresh = None;
        }

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

        self.last_game_tick = Some(game.ticks);

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

fn confirmed_history_context(session: &HistorySession) -> bool {
    let accounts: std::collections::BTreeSet<_> = session
        .participants
        .iter()
        .filter(|member| !member.is_ai)
        .filter_map(|member| member.steam_id)
        .collect();
    session
        .local_steam_id
        .is_some_and(|local| accounts.contains(&local))
        && accounts.len() >= 2
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

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::engine::memory::MemoryRead;

    #[test]
    #[ignore = "requires a running initialized game; reads only, never creates journal files"]
    fn live_history_session_is_read_only() -> anyhow::Result<()> {
        let mut source = LiveStatsSource::new();
        source.process = Some(attach_to_known_game_process()?);
        let process = source.process.as_ref().unwrap();
        let player_before = process.read_u32(process.module_addr(UI_PLAYER_ID))?;
        let sample = source.history_sample()?;
        let humans = sample
            .session
            .participants
            .iter()
            .filter(|member| !member.is_ai && member.steam_id.is_some())
            .count();
        let terminal_metadata = source
            .multiplayer_runtime
            .as_ref()
            .is_some_and(Runtime::has_terminal_metadata);
        println!("History reader: multiplayer={}, director_multiplayer={}, loaded={}, completed={}, end-screen metadata={terminal_metadata}, Steam accounts={humans}",sample.session.multiplayer_active,sample.session.director_multiplayer,sample.session.world_loaded,sample.session.completed);
        if sample.session.world_loaded {
            assert_eq!(sample.session.game_type, Some(3));
            assert!(sample.snapshot.game.is_plausible());
        }
        let process = source.process.as_ref().unwrap();
        assert_eq!(
            player_before,
            process.read_u32(process.module_addr(UI_PLAYER_ID))?
        );
        Ok(())
    }

    #[test]
    #[ignore = "requires a running game; read-only, also checks the no-multiplayer fallback"]
    fn live_multiplayer_import_is_read_only() -> anyhow::Result<()> {
        let mut source = LiveStatsSource::new();
        source.process = Some(attach_to_known_game_process()?);
        let process = source.process.as_ref().unwrap();
        let before = process.read_u32(process.module_addr(UI_PLAYER_ID))?;
        let started = Instant::now();
        let result = source.multiplayer_players();
        let process = source.process.as_ref().unwrap();
        assert_eq!(before, process.read_u32(process.module_addr(UI_PLAYER_ID))?);
        let players = match result {
            Ok(players) => players,
            Err(error) if error.to_string().starts_with("No multiplayer ") => {
                println!("No current multiplayer session: import returned an error without changing the selected player.");
                assert!(source.multiplayer_runtime.is_none());
                assert!(source.last_error.is_none());
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        println!(
            "Imported {} names, {} colors and {} SteamIDs in {:?}",
            players.name_count(),
            players.color_count(),
            players
                .players
                .values()
                .filter(|player| player.steam_id.is_some())
                .count(),
            started.elapsed()
        );
        assert!(source.multiplayer_runtime.is_some());
        let second = source.multiplayer_players()?;
        assert_eq!(players.appearance([1; 8]), second.appearance([1; 8]));
        for (slot, player) in &players.players {
            assert_eq!(player.steam_id, second.players.get(slot).unwrap().steam_id);
        }
        Ok(())
    }

    #[test]
    #[ignore = "explicit opt-in: records the current multiplayer match in the authorized local journal"]
    fn live_history_creates_and_updates_one_json() -> anyhow::Result<()> {
        if std::env::var("SHCDE_HISTORY_LIVE_CAPTURE").as_deref() != Ok("1") {
            return Ok(());
        }
        let config: crate::config::AppConfig =
            serde_json::from_slice(&std::fs::read(crate::config::config_path())?)?;
        anyhow::ensure!(
            config.admin_mode_enabled(),
            "Admin unlock required for live journal capture"
        );
        let mut source = LiveStatsSource::new();
        let sample = source.history_sample()?;
        anyhow::ensure!(
            confirmed_history_context(&sample.session) && sample.session.world_loaded,
            "A confirmed online multiplayer match is required"
        );
        let root = crate::history::directory();
        let mut recorder = crate::history::recorder::Recorder::open(root.clone())?;
        recorder.observe(&sample)?;
        let id = recorder
            .active()
            .ok_or_else(|| anyhow::anyhow!("Match did not enter recording"))?
            .id
            .clone();
        let path = crate::history::game_path(&root, &id)?;
        let first = std::fs::read(&path)?;
        for _ in 0..6 {
            std::thread::sleep(Duration::from_secs(1));
            let sample = source.history_sample()?;
            recorder.observe(&sample)?;
        }
        recorder.checkpoint()?;
        let saved = crate::history::read_record(&path)?;
        assert_eq!(saved.players.len(), 8);
        assert_ne!(
            first,
            std::fs::read(&path)?,
            "Latest-state JSON should update"
        );
        let records = crate::history::load_games(&root)?.0;
        let journal = recorder.rebuild_journal(&records)?;
        println!("Live recording verified: one match JSON updated; {} Steam accounts indexed. Game and controls unchanged.",journal.players.len());
        Ok(())
    }
}
