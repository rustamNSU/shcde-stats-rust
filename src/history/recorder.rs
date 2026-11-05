//! Keeps one latest-state JSON per confirmed multiplayer match.
use std::{
    cell::RefCell,
    collections::BTreeSet,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{bail, ensure, Context, Result};
use serde_json::Value;

use crate::engine::{
    building::aggregate_buildings,
    history_session::{HistorySession, SessionParticipant},
    service::HistorySample,
    snapshot::overlay_rows,
    unit::aggregate_units,
};

use super::{
    build_journal, load_games, new_id, now, recorder_id, valid_steam_id, GameRecord, HistoryWriter,
    Participant, PlayerJournal, FORMAT_VERSION,
};

const SAVE_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
enum EndKind {
    Interrupted,
    Left,
    Completed,
}

pub struct Recorder {
    writer: HistoryWriter,
    root: PathBuf,
    recorder_id: String,
    active: Option<GameRecord>,
    pending: Vec<GameRecord>,
    finalized_worlds: BTreeSet<String>,
    active_since: Option<Instant>,
    observed_before_resume: u64,
    last_save: Option<Instant>,
    resync_pending: bool,
    observed_inactive: bool,
    message: String,
    last_error: RefCell<Option<String>>,
}

impl Recorder {
    pub fn open(root: PathBuf) -> Result<Self> {
        let writer = HistoryWriter::start(root.clone())?;
        let recorder_id = recorder_id(&root)?;
        let (games, invalid_files) = load_games(&root)?;
        let pending = games
            .iter()
            .filter(|game| game.recorder_id == recorder_id && !game.finalized())
            .cloned()
            .collect();
        let finalized_worlds = games
            .iter()
            .filter(|game| game.recorder_id == recorder_id && game.finalized())
            .map(|game| game.world_key.clone())
            .collect();
        Ok(Self {
            writer,
            root,
            recorder_id,
            active: None,
            pending,
            finalized_worlds,
            active_since: None,
            observed_before_resume: 0,
            last_save: None,
            resync_pending: false,
            observed_inactive: false,
            message: if invalid_files == 0 {
                "Waiting for a confirmed multiplayer match.".into()
            } else {
                format!(
                    "Waiting for multiplayer. {invalid_files} unreadable history files were kept."
                )
            },
            last_error: RefCell::new(None),
        })
    }

    pub fn recorder_id(&self) -> &str {
        &self.recorder_id
    }

    pub fn active(&self) -> Option<&GameRecord> {
        self.active.as_ref()
    }

    pub fn status(&self) -> String {
        let _ = self.poll_writer_errors();
        if let Some(error) = self.last_error.borrow().as_ref() {
            return format!("History save failed: {error}");
        }
        self.message.clone()
    }

    pub fn observe(&mut self, sample: &HistorySample) -> Result<()> {
        self.observe_at(sample, Instant::now())
    }

    fn observe_at(&mut self, sample: &HistorySample, instant: Instant) -> Result<()> {
        self.poll_writer_errors()?;
        let session = &sample.session;
        if session.completed {
            return self.complete_at(sample, instant);
        }
        if session.resyncing {
            self.resync_pending = true;
            self.message = "Multiplayer resync: keeping the last valid recording.".into();
            self.save_if_due(instant)?;
            return Ok(());
        }
        if !session.multiplayer_active || !session.director_multiplayer {
            self.finalize_active("Left the multiplayer match", EndKind::Left)?;
            self.finalize_pending("Previous multiplayer match is no longer active")?;
            self.observed_inactive = true;
            self.message = "Waiting for a confirmed multiplayer match.".into();
            return Ok(());
        }
        if !session.world_loaded || !sample.snapshot.game.is_plausible() {
            self.message = "Waiting for a valid multiplayer world; previous data was kept.".into();
            self.save_if_due(instant)?;
            return Ok(());
        }
        validate_slots(session)?;
        let Some(world_key) = world_key(session)? else {
            self.message = "Waiting for the multiplayer match identity.".into();
            return Ok(());
        };

        let same_world = self
            .active
            .as_ref()
            .is_some_and(|record| record.world_key == world_key);
        let resync_continuation = self.resync_pending
            && self.active.as_ref().is_some_and(|record| {
                record.process_id == session.process_id && compatible_roster(record, session)
            });
        if self.active.is_some() && !same_world && !resync_continuation {
            self.finalize_active(
                "A different multiplayer round was observed",
                EndKind::Interrupted,
            )?;
        }
        if resync_continuation {
            if let Some(record) = self.active.as_mut() {
                record.world_key = world_key.clone();
            }
        }
        self.resync_pending = false;

        if self.active.is_none() {
            if let Some(index) = self
                .pending
                .iter()
                .position(|game| !game.finalized() && game.world_key == world_key)
            {
                let record = self.pending.remove(index);
                self.observed_before_resume = record.observed_seconds;
                self.active = Some(record);
            } else {
                if self.finalized_worlds.contains(&world_key) && !self.observed_inactive {
                    self.message =
                        "This match is already finalized; waiting for the next match.".into();
                    return Ok(());
                }
                if !confirmed_roster(session) {
                    self.message =
                        "Waiting for this account and another human Steam player.".into();
                    return Ok(());
                }
                self.active = Some(new_record(&self.recorder_id, &world_key, sample)?);
                self.observed_before_resume = 0;
            }
            self.active_since = Some(instant);
            self.observed_inactive = false;
            self.last_save = None;
            self.finalize_pending("A different multiplayer match was observed after restart")?;
            self.update_active(sample, instant)?;
            // Ensure the one record exists as soon as the start is confirmed.
            self.queue_active()?;
            self.flush()?;
            self.last_save = Some(instant);
        } else {
            self.update_active(sample, instant)?;
            self.save_if_due(instant)?;
        }
        if let Some(record) = self.active.as_ref() {
            self.message = format!(
                "Recording {} · tick {} · the same JSON is saved every 5 seconds.",
                record.map_name.as_deref().unwrap_or("multiplayer match"),
                record.last_tick
            );
        }
        Ok(())
    }

    pub fn unavailable(&mut self, process_running: bool) -> Result<()> {
        self.poll_writer_errors()?;
        if process_running {
            self.message =
                "Game reading is temporarily unavailable; last valid data was kept.".into();
            self.save_if_due(Instant::now())?;
        } else {
            self.finalize_active("Game process exited", EndKind::Interrupted)?;
            self.finalize_pending("Game process exited before recording could resume")?;
            self.observed_inactive = true;
            self.message = "Game closed. Waiting for the next multiplayer match.".into();
        }
        Ok(())
    }

    /// Saves the latest RAM snapshot while leaving this match resumable.
    pub fn checkpoint(&self) -> Result<()> {
        self.queue_active()?;
        self.flush()
    }

    /// Explicitly abandons capture and freezes any unfinished records.
    #[cfg(test)]
    pub fn finish(&mut self, reason: &str) -> Result<()> {
        self.finalize_active(reason, EndKind::Interrupted)?;
        self.finalize_pending(reason)?;
        self.flush()
    }

    pub fn rebuild_journal(&self, games: &[GameRecord]) -> Result<PlayerJournal> {
        self.flush()?;
        let journal = build_journal(games, &self.recorder_id);
        self.writer.journal(journal.clone())?;
        self.flush()?;
        Ok(journal)
    }

    pub fn flush(&self) -> Result<()> {
        self.writer.flush()?;
        self.poll_writer_errors()?;
        *self.last_error.borrow_mut() = None;
        Ok(())
    }

    fn poll_writer_errors(&self) -> Result<()> {
        if let Some(error) = self.writer.error() {
            *self.last_error.borrow_mut() = Some(error.clone());
            bail!("{error}");
        }
        Ok(())
    }

    fn complete_at(&mut self, sample: &HistorySample, instant: Instant) -> Result<()> {
        validate_slots(&sample.session)?;
        let key = world_key(&sample.session)?;
        if self.active.is_none() {
            if let Some(index) = self.pending.iter().position(|game| {
                !game.finalized()
                    && game.process_id == sample.session.process_id
                    && key.as_ref() == Some(&game.world_key)
            }) {
                let record = self.pending.remove(index);
                self.observed_before_resume = record.observed_seconds;
                self.active_since = Some(instant);
                self.active = Some(record);
            }
        }
        let matches_active = self.active.as_ref().is_some_and(|record| {
            record.process_id == sample.session.process_id
                && (key.as_ref() == Some(&record.world_key)
                    || (self.resync_pending && compatible_roster(record, &sample.session)))
        });
        if !matches_active {
            self.message = "Match ended; waiting for the next multiplayer match.".into();
            return Ok(());
        }
        if sample.snapshot.game.is_plausible() {
            self.update_active(sample, instant)?;
        } else if let Some(record) = self.active.as_mut() {
            merge_results(record, &sample.session);
        }
        self.finalize_active("Game end was confirmed by the game", EndKind::Completed)?;
        self.finalize_pending("An older recording could not resume")?;
        Ok(())
    }

    fn update_active(&mut self, sample: &HistorySample, instant: Instant) -> Result<()> {
        let Some(record) = self.active.as_mut() else {
            return Ok(());
        };
        record.updated_at = now();
        record.last_tick = sample.snapshot.game.ticks;
        record.observed_seconds = self.observed_before_resume.saturating_add(
            self.active_since
                .map(|start| instant.saturating_duration_since(start).as_secs())
                .unwrap_or_default(),
        );
        if record.map_name.is_none() {
            record.map_name = sample.session.map_name.clone();
        }
        if record.map_size.is_none() {
            record.map_size = sample.session.map_size;
        }
        for (key, value) in &sample.session.setup {
            record
                .setup
                .entry(key.clone())
                .or_insert_with(|| value.clone());
        }
        let host_known = record.players.iter().any(|player| player.is_host);
        for member in &sample.session.participants {
            merge_member(&mut record.players[member.slot - 1], member, !host_known);
        }
        for player in &mut record.players {
            if player.steam_id.is_some() && player.steam_id == record.local_steam_id {
                player.is_self = true;
            }
        }
        merge_results(record, &sample.session);
        let unit_counts = aggregate_units(&sample.snapshot.units);
        let building_counts = aggregate_buildings(&sample.snapshot.buildings);
        for stats in overlay_rows(&sample.snapshot) {
            let player_id = stats.player_id;
            if let Some(player) = usize::from(stats.player_id)
                .checked_sub(1)
                .and_then(|index| record.players.get_mut(index))
            {
                if player.kind == "empty" {
                    player.statistics = Value::Null;
                    continue;
                }
                let cumulative = sample
                    .snapshot
                    .post_game_statistics
                    .players
                    .iter()
                    .find(|stats| stats.player_id == u32::from(player_id))
                    .map(serde_json::to_value)
                    .transpose()?
                    .unwrap_or_else(|| player.statistics["cumulative"].clone());
                let mut statistics = serde_json::to_value(stats)?;
                let fields = statistics
                    .as_object_mut()
                    .context("Player statistics must be a JSON object")?;
                fields.insert("cumulative".into(), cumulative);
                fields.insert(
                    "unit_counts_by_type".into(),
                    unit_counts
                        .get(&player_id)
                        .map(|counts| serde_json::to_value(&counts.units_by_type))
                        .transpose()?
                        .unwrap_or_else(|| Value::Object(Default::default())),
                );
                fields.insert(
                    "building_counts_by_type".into(),
                    building_counts
                        .get(&player_id)
                        .map(|counts| serde_json::to_value(&counts.buildings_by_type))
                        .transpose()?
                        .unwrap_or_else(|| Value::Object(Default::default())),
                );
                player.statistics = statistics;
            }
        }
        Ok(())
    }

    fn save_if_due(&mut self, instant: Instant) -> Result<()> {
        if self.active.is_some()
            && self
                .last_save
                .is_none_or(|last| instant.saturating_duration_since(last) >= SAVE_INTERVAL)
        {
            self.queue_active()?;
            self.last_save = Some(instant);
        }
        Ok(())
    }

    fn queue_active(&self) -> Result<()> {
        if let Some(record) = self.active.as_ref() {
            self.writer.record(record.clone()).with_context(|| {
                format!("Could not queue a game record in {}", self.root.display())
            })?;
        }
        Ok(())
    }

    fn finalize_active(&mut self, reason: &str, kind: EndKind) -> Result<()> {
        let Some(active) = self.active.as_ref() else {
            return Ok(());
        };
        let mut final_record = active.clone();
        let winner_seen = final_record
            .players
            .iter()
            .any(|player| player.result.as_deref() == Some("win"));
        final_record.status = match kind {
            EndKind::Interrupted => "interrupted",
            EndKind::Completed => "finished",
            EndKind::Left if winner_seen => "finished",
            EndKind::Left => "left",
        }
        .into();
        final_record.updated_at = now();
        final_record.ended_at = Some(final_record.updated_at.clone());
        final_record.finish_reason = Some(reason.into());
        // Drain preceding writes before checking the final write acknowledgement.
        self.writer.flush()?;
        let _ = self.poll_writer_errors();
        self.writer.record(final_record.clone())?;
        self.flush()?;
        self.finalized_worlds.insert(final_record.world_key);
        self.active = None;
        self.active_since = None;
        self.last_save = None;
        self.resync_pending = false;
        self.message = format!("Recording finalized: {reason}.");
        Ok(())
    }

    fn finalize_pending(&mut self, reason: &str) -> Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }
        for record in &mut self.pending {
            if !record.finalized() {
                record.status = "interrupted".into();
                record.updated_at = now();
                record.ended_at = Some(record.updated_at.clone());
                record.finish_reason = Some(reason.into());
            }
            self.writer.record(record.clone())?;
        }
        self.flush()?;
        self.finalized_worlds
            .extend(self.pending.iter().map(|game| game.world_key.clone()));
        self.pending.clear();
        Ok(())
    }
}

fn merge_results(record: &mut GameRecord, session: &HistorySession) {
    for (slot, result) in &session.results {
        if let Some(player) = slot
            .checked_sub(1)
            .and_then(|index| record.players.get_mut(index))
        {
            if player.kind != "empty" && matches!(result.as_str(), "win" | "loss") {
                player.result = Some(result.clone());
            }
        }
    }
}

fn world_key(session: &HistorySession) -> Result<Option<String>> {
    let Some(token) = session
        .game_token
        .as_deref()
        .filter(|token| !token.is_empty())
    else {
        return Ok(None);
    };
    ensure!(token.len() <= 1_900, "Multiplayer identity is too long");
    Ok(Some(format!("{}:{token}", session.process_id)))
}

fn validate_slots(session: &HistorySession) -> Result<()> {
    let mut slots = BTreeSet::new();
    ensure!(
        session.participants.len() <= 8,
        "Too many multiplayer players"
    );
    for participant in &session.participants {
        ensure!(
            (1..=8).contains(&participant.slot) && slots.insert(participant.slot),
            "Invalid or repeated multiplayer player slot"
        );
    }
    Ok(())
}

fn account(member: &SessionParticipant) -> Option<String> {
    (!member.is_ai)
        .then_some(member.steam_id)
        .flatten()
        .map(|id| id.to_string())
        .filter(|id| valid_steam_id(id))
}

fn self_account(session: &HistorySession) -> Option<String> {
    let local = session
        .local_steam_id
        .map(|id| id.to_string())
        .filter(|id| valid_steam_id(id));
    session.participants.iter().find_map(|member| {
        let id = account(member)?;
        (member.is_self || local.as_ref() == Some(&id)).then_some(id)
    })
}

fn confirmed_roster(session: &HistorySession) -> bool {
    let accounts: BTreeSet<_> = session.participants.iter().filter_map(account).collect();
    self_account(session).is_some() && accounts.len() >= 2
}

fn compatible_roster(record: &GameRecord, session: &HistorySession) -> bool {
    if let Some(local) = self_account(session) {
        if record
            .local_steam_id
            .as_ref()
            .is_some_and(|old| old != &local)
        {
            return false;
        }
    }
    session.participants.iter().all(|member| {
        account(member)
            .is_none_or(|id| record.players[member.slot - 1].steam_id.as_ref() == Some(&id))
    })
}

fn merge_member(player: &mut Participant, member: &SessionParticipant, allow_host: bool) {
    let incoming_id = account(member);
    // A departing player or a reused UI slot must not replace the original actor.
    if player.steam_id.is_some() && player.steam_id != incoming_id {
        return;
    }
    if let Some(id) = incoming_id {
        player.kind = "human".into();
        player.steam_id = Some(id);
    } else if member.is_ai && player.kind != "human" {
        player.kind = "ai".into();
    } else if player.kind == "empty" && member.name.is_some() {
        player.kind = "unknown".into();
    }
    if let Some(name) = member.name.as_ref().filter(|name| !name.trim().is_empty()) {
        player.steam_name = Some(name.chars().take(512).collect());
    }
    if let Some(color) = member.color.filter(|color| (1..=8).contains(color)) {
        player.color = Some(color);
    }
    if member.team.is_some() {
        player.team = member.team;
    }
    player.is_self |= member.is_self;
    player.is_host |= member.is_host && allow_host;
}

fn new_record(recorder: &str, key: &str, sample: &HistorySample) -> Result<GameRecord> {
    let timestamp = now();
    let mut setup = sample.session.setup.clone();
    if let Some(game_type) = sample.session.game_type {
        setup
            .entry("game_type".into())
            .or_insert(Value::from(game_type));
    }
    setup
        .entry("loaded_from_save".into())
        .or_insert(Value::from(sample.session.loading_from_save));
    Ok(GameRecord {
        schema_version: FORMAT_VERSION,
        id: new_id()?,
        recorder_id: recorder.into(),
        app_version: env!("CARGO_PKG_VERSION").into(),
        started_at: timestamp.clone(),
        updated_at: timestamp,
        ended_at: None,
        status: "recording".into(),
        finish_reason: None,
        process_id: sample.session.process_id,
        lobby_id: sample.session.lobby_id.map(|id| id.to_string()),
        local_steam_id: self_account(&sample.session),
        world_key: key.into(),
        first_tick: sample.snapshot.game.ticks,
        last_tick: sample.snapshot.game.ticks,
        observed_seconds: 0,
        partial: sample.session.loading_from_save || sample.snapshot.game.ticks > 0,
        map_name: sample.session.map_name.clone(),
        map_size: sample.session.map_size,
        setup,
        players: (1..=8)
            .map(|slot| Participant {
                slot,
                kind: "empty".into(),
                steam_id: None,
                steam_name: None,
                color: None,
                team: None,
                is_self: false,
                is_host: false,
                result: None,
                statistics: Value::Null,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use super::*;
    use crate::{
        engine::{
            building::BuildingSnapshot, game::PostGamePlayerStats, service::empty_snapshot,
            unit::UnitSnapshot,
        },
        history::{game_path, read_record},
    };

    const SELF: u64 = 76_561_198_027_391_269;
    const OTHER: u64 = SELF + 1;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Result<Self> {
            Ok(Self(
                std::env::temp_dir().join(format!("shc-recorder-{}", new_id()?)),
            ))
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn fixture(token: &str, tick: u32) -> HistorySample {
        let mut snapshot = empty_snapshot();
        snapshot.game.year = 1181;
        snapshot.game.month = 6;
        snapshot.game.ticks = tick;
        snapshot.players[0].gold = 100;
        snapshot.players[1].gold = 200;
        HistorySample {
            snapshot,
            session: HistorySession {
                multiplayer_active: true,
                director_multiplayer: true,
                world_loaded: true,
                process_id: 42,
                game_token: Some(token.into()),
                local_steam_id: Some(SELF),
                map_name: Some("Test map".into()),
                map_size: Some(400),
                game_type: Some(3),
                participants: vec![
                    SessionParticipant {
                        slot: 1,
                        steam_id: Some(SELF),
                        name: Some("Self".into()),
                        color: Some(6),
                        is_self: true,
                        is_host: true,
                        team: Some(1),
                        ..SessionParticipant::default()
                    },
                    SessionParticipant {
                        slot: 2,
                        steam_id: Some(OTHER),
                        name: Some("Other".into()),
                        color: Some(5),
                        team: Some(2),
                        ..SessionParticipant::default()
                    },
                ],
                ..HistorySession::default()
            },
        }
    }

    fn game_count(root: &Path) -> Result<usize> {
        Ok(load_games(root)?.0.len())
    }

    #[test]
    fn creates_immediately_updates_the_same_file_at_five_seconds_then_freezes() -> Result<()> {
        let directory = TestDirectory::new()?;
        let mut recorder = Recorder::open(directory.0.clone())?;
        let start = Instant::now();
        let mut sample = fixture("round-a", 0);
        recorder.observe_at(&sample, start)?;
        let id = recorder.active().expect("recording").id.clone();
        let path = game_path(&directory.0, &id)?;
        let initial_bytes = fs::read(&path)?;
        assert_eq!(read_record(&path)?.players.len(), 8);

        sample.snapshot.game.ticks = 40;
        sample.snapshot.players[0].gold = 900;
        recorder.observe_at(&sample, start + Duration::from_secs(4))?;
        recorder.flush()?;
        assert_eq!(
            recorder.active().expect("recording").players[0].statistics["gold"],
            900
        );
        assert_eq!(
            fs::read(&path)?,
            initial_bytes,
            "RAM refresh must not rewrite early"
        );

        // A paused native tick must not stop the wall-clock disk cadence.
        sample.snapshot.players[0].gold = 950;
        recorder.observe_at(&sample, start + Duration::from_secs(5))?;
        recorder.flush()?;
        assert_eq!(read_record(&path)?.players[0].statistics["gold"], 950);
        assert_eq!(game_count(&directory.0)?, 1);

        recorder.finish("Monitor closed")?;
        let final_bytes = fs::read(&path)?;
        assert_eq!(read_record(&path)?.status, "interrupted");
        sample.snapshot.players[0].gold = 2_000;
        recorder.observe_at(&sample, start + Duration::from_secs(10))?;
        recorder.flush()?;
        assert!(recorder.active().is_none());
        assert_eq!(fs::read(&path)?, final_bytes);
        assert_eq!(game_count(&directory.0)?, 1);
        Ok(())
    }

    #[test]
    fn checkpoint_saves_latest_ram_without_finalizing_and_restart_resumes() -> Result<()> {
        let directory = TestDirectory::new()?;
        let start = Instant::now();
        let id;
        {
            let mut recorder = Recorder::open(directory.0.clone())?;
            let mut sample = fixture("round-a", 100);
            recorder.observe_at(&sample, start)?;
            id = recorder.active().expect("recording").id.clone();
            let path = game_path(&directory.0, &id)?;
            sample.snapshot.players[0].gold = 777;
            sample.snapshot.game.ticks = 150;
            recorder.observe_at(&sample, start + Duration::from_secs(1))?;
            assert_eq!(read_record(&path)?.players[0].statistics["gold"], 100);
            recorder.checkpoint()?;
            let saved = read_record(&path)?;
            assert_eq!(saved.players[0].statistics["gold"], 777);
            assert_eq!(saved.last_tick, 150);
            assert_eq!(saved.status, "recording");
            assert!(saved.ended_at.is_none());
        }
        let mut recorder = Recorder::open(directory.0.clone())?;
        let mut sample = fixture("round-a", 200);
        sample.snapshot.players[0].gold = 888;
        recorder.observe_at(&sample, start + Duration::from_secs(10))?;
        assert_eq!(recorder.active().expect("resumed").id, id);
        assert_eq!(game_count(&directory.0)?, 1);
        recorder.checkpoint()?;
        Ok(())
    }

    #[test]
    fn saves_cumulative_and_all_type_counts_without_replacing_initial_setup() -> Result<()> {
        let directory = TestDirectory::new()?;
        let mut recorder = Recorder::open(directory.0.clone())?;
        let start = Instant::now();
        let mut sample = fixture("round-a", 100);
        sample
            .session
            .setup
            .insert("starting_gold".into(), 2_000.into());
        sample.snapshot.post_game_statistics.players = vec![PostGamePlayerStats {
            player_id: 1,
            army_recruited: 123,
            max_pop_in_game: 90,
            destroyed_buildings: 7,
            ..PostGamePlayerStats::default()
        }];
        sample.snapshot.units = [(1, 77), (1, 77), (1, 80), (2, 80)]
            .into_iter()
            .map(|(player_id, type_id)| UnitSnapshot {
                player_id,
                type_id,
                ..UnitSnapshot::default()
            })
            .collect();
        sample.snapshot.buildings = [(1, 87), (1, 87), (2, 87)]
            .into_iter()
            .map(|(owner_player_id, type_id)| BuildingSnapshot {
                owner_player_id,
                type_id,
                ..BuildingSnapshot::default()
            })
            .collect();
        recorder.observe_at(&sample, start)?;
        let id = recorder.active().expect("recording").id.clone();
        let saved = read_record(&game_path(&directory.0, &id)?)?;
        let statistics = &saved.players[0].statistics;
        assert_eq!(statistics["cumulative"]["army_recruited"], 123);
        assert_eq!(statistics["cumulative"]["max_pop_in_game"], 90);
        assert_eq!(statistics["cumulative"]["destroyed_buildings"], 7);
        assert_eq!(
            statistics["unit_counts_by_type"],
            serde_json::json!({"77": 2, "80": 1})
        );
        assert_eq!(
            statistics["building_counts_by_type"],
            serde_json::json!({"87": 2})
        );
        assert_eq!(saved.players[1].statistics["unit_counts_by_type"]["80"], 1);
        assert!(saved.players[2].statistics.is_null());

        sample
            .session
            .setup
            .insert("starting_gold".into(), 9_999.into());
        sample.snapshot.post_game_statistics.players.clear();
        sample.snapshot.units.clear();
        sample.snapshot.buildings.clear();
        recorder.observe_at(&sample, start + Duration::from_secs(1))?;
        recorder.checkpoint()?;
        let saved = read_record(&game_path(&directory.0, &id)?)?;
        assert_eq!(saved.setup["starting_gold"], 2_000);
        assert_eq!(
            saved.players[0].statistics["cumulative"]["army_recruited"],
            123
        );
        assert_eq!(
            saved.players[0].statistics["unit_counts_by_type"],
            serde_json::json!({})
        );
        assert_eq!(
            saved.players[0].statistics["building_counts_by_type"],
            serde_json::json!({})
        );
        assert!(saved.players[0].statistics.get("units").is_none());
        assert!(saved.players[0]
            .statistics
            .get("buildings_records")
            .is_none());
        Ok(())
    }

    #[test]
    fn resumes_owned_unfinished_record_and_splits_a_new_round() -> Result<()> {
        let directory = TestDirectory::new()?;
        let start = Instant::now();
        let first_id;
        let recorder_id;
        {
            let mut recorder = Recorder::open(directory.0.clone())?;
            recorder.observe_at(&fixture("round-a", 100), start)?;
            first_id = recorder.active().expect("recording").id.clone();
            recorder_id = recorder.recorder_id().to_owned();
            // Dropping without finish represents an interrupted monitor process.
        }
        let mut recorder = Recorder::open(directory.0.clone())?;
        assert_eq!(recorder.recorder_id(), recorder_id);
        recorder.observe_at(&fixture("round-a", 200), start + Duration::from_secs(10))?;
        assert_eq!(recorder.active().expect("resumed").id, first_id);
        assert_eq!(game_count(&directory.0)?, 1);

        recorder.observe_at(&fixture("round-b", 0), start + Duration::from_secs(11))?;
        assert_ne!(recorder.active().expect("new round").id, first_id);
        let original = read_record(&game_path(&directory.0, &first_id)?)?;
        assert_eq!(original.status, "interrupted");
        assert_eq!(original.last_tick, 200);
        assert_eq!(game_count(&directory.0)?, 2);
        recorder.finish("Monitor closed")?;
        Ok(())
    }

    #[test]
    fn keeps_departed_roster_and_last_valid_values_through_read_failure() -> Result<()> {
        let directory = TestDirectory::new()?;
        let mut recorder = Recorder::open(directory.0.clone())?;
        let start = Instant::now();
        let mut sample = fixture("round-a", 100);
        recorder.observe_at(&sample, start)?;
        let id = recorder.active().expect("recording").id.clone();
        sample.session.participants.truncate(1);
        sample.snapshot.players[0].gold = 700;
        recorder.observe_at(&sample, start + Duration::from_secs(1))?;
        recorder.unavailable(true)?;
        let active = recorder.active().expect("temporary failure is not an end");
        assert_eq!(
            active.players[1].steam_id.as_deref(),
            Some(OTHER.to_string().as_str())
        );
        assert_eq!(active.players[0].statistics["gold"], 700);

        recorder.unavailable(false)?;
        let saved = read_record(&game_path(&directory.0, &id)?)?;
        assert_eq!(saved.status, "interrupted");
        assert_eq!(saved.players[1].steam_name.as_deref(), Some("Other"));
        assert_eq!(saved.players[0].statistics["gold"], 700);
        Ok(())
    }

    #[test]
    fn excludes_offline_lobby_ai_only_and_unidentified_self() -> Result<()> {
        let directory = TestDirectory::new()?;
        let mut recorder = Recorder::open(directory.0.clone())?;
        let start = Instant::now();
        let mut offline = fixture("offline", 100);
        offline.session.multiplayer_active = false;
        recorder.observe_at(&offline, start)?;
        let mut lobby = fixture("lobby", 100);
        lobby.session.world_loaded = false;
        recorder.observe_at(&lobby, start)?;
        let mut ai_only = fixture("ai", 100);
        ai_only.session.participants[1].is_ai = true;
        ai_only.session.participants[1].steam_id = None;
        recorder.observe_at(&ai_only, start)?;
        let mut unidentified = fixture("unknown", 100);
        unidentified.session.local_steam_id = None;
        unidentified.session.participants[0].is_self = false;
        recorder.observe_at(&unidentified, start)?;
        assert!(recorder.active().is_none());
        assert_eq!(game_count(&directory.0)?, 0);
        Ok(())
    }

    #[test]
    fn observed_resync_can_replace_roster_object_without_creating_another_game() -> Result<()> {
        let directory = TestDirectory::new()?;
        let mut recorder = Recorder::open(directory.0.clone())?;
        let start = Instant::now();
        let mut sample = fixture("before-resync", 100);
        sample.session.loading_from_save = true;
        recorder.observe_at(&sample, start)?;
        let id = recorder
            .active()
            .expect("saved worlds can record")
            .id
            .clone();
        sample.session.resyncing = true;
        sample.session.game_token = Some("after-resync".into());
        sample.snapshot = empty_snapshot();
        recorder.observe_at(&sample, start + Duration::from_secs(1))?;
        sample.snapshot = fixture("unused", 90).snapshot;
        sample.session.resyncing = false;
        recorder.observe_at(&sample, start + Duration::from_secs(2))?;
        assert_eq!(recorder.active().expect("same match").id, id);
        assert_eq!(game_count(&directory.0)?, 1);

        sample.session.results.insert(1, "win".into());
        recorder.observe_at(&sample, start + Duration::from_secs(3))?;
        assert_eq!(
            recorder
                .active()
                .expect("winner alone is not an end")
                .status,
            "recording"
        );
        sample.session.multiplayer_active = false;
        sample.session.director_multiplayer = false;
        sample.session.world_loaded = false;
        sample.session.participants.clear();
        sample.snapshot = empty_snapshot();
        recorder.observe_at(&sample, start + Duration::from_secs(4))?;
        let saved = read_record(&game_path(&directory.0, &id)?)?;
        assert_eq!(saved.status, "finished");
        assert_eq!(saved.last_tick, 90);
        assert_eq!(saved.players[0].result.as_deref(), Some("win"));
        assert_eq!(saved.players[1].steam_name.as_deref(), Some("Other"));
        Ok(())
    }

    #[test]
    fn confirmed_end_saves_terminal_values_without_requiring_a_known_winner() -> Result<()> {
        let directory = TestDirectory::new()?;
        let mut recorder = Recorder::open(directory.0.clone())?;
        let start = Instant::now();
        let mut sample = fixture("round-a", 100);
        recorder.observe_at(&sample, start)?;
        let id = recorder.active().expect("recording").id.clone();
        sample.session.completed = true;
        sample.session.world_loaded = false;
        sample.session.participants.clear();
        sample.snapshot.players[0].gold = 1_234;
        sample.snapshot.game.ticks = 200;
        recorder.observe_at(&sample, start + Duration::from_secs(1))?;
        let path = game_path(&directory.0, &id)?;
        let saved = read_record(&path)?;
        assert_eq!(saved.status, "finished");
        assert_eq!(saved.players[0].statistics["gold"], 1_234);
        assert_eq!(saved.players[0].steam_name.as_deref(), Some("Self"));
        assert!(saved.players.iter().all(|player| player.result.is_none()));
        let final_bytes = fs::read(&path)?;
        sample.snapshot = empty_snapshot();
        recorder.observe_at(&sample, start + Duration::from_secs(2))?;
        assert_eq!(fs::read(path)?, final_bytes);
        assert_eq!(game_count(&directory.0)?, 1);
        Ok(())
    }

    #[test]
    fn confirmed_end_can_finalize_the_owned_record_after_monitor_restart() -> Result<()> {
        let directory = TestDirectory::new()?;
        let start = Instant::now();
        let id;
        {
            let mut recorder = Recorder::open(directory.0.clone())?;
            recorder.observe_at(&fixture("round-a", 100), start)?;
            id = recorder.active().expect("recording").id.clone();
        }
        let mut recorder = Recorder::open(directory.0.clone())?;
        let mut sample = fixture("round-a", 200);
        sample.session.completed = true;
        sample.session.world_loaded = false;
        recorder.observe_at(&sample, start + Duration::from_secs(10))?;
        assert!(recorder.active().is_none());
        assert_eq!(
            read_record(&game_path(&directory.0, &id)?)?.status,
            "finished"
        );
        assert_eq!(game_count(&directory.0)?, 1);
        Ok(())
    }
}
