//! One mutable JSON per running match; completed match files are immutable.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const FORMAT_VERSION: u32 = 1;
pub mod recorder;
const MAX_RECORD_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARCHIVE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Participant {
    pub slot: usize,
    pub kind: String,
    pub steam_id: Option<String>,
    pub steam_name: Option<String>,
    pub color: Option<i32>,
    pub team: Option<i32>,
    pub is_self: bool,
    pub is_host: bool,
    pub result: Option<String>,
    pub statistics: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameRecord {
    pub schema_version: u32,
    pub id: String,
    pub recorder_id: String,
    pub app_version: String,
    pub started_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub finish_reason: Option<String>,
    pub process_id: u32,
    pub lobby_id: Option<String>,
    pub local_steam_id: Option<String>,
    pub world_key: String,
    pub first_tick: u32,
    pub last_tick: u32,
    pub observed_seconds: u64,
    pub partial: bool,
    pub map_name: Option<String>,
    pub map_size: Option<u32>,
    pub setup: BTreeMap<String, Value>,
    pub players: Vec<Participant>,
}

impl GameRecord {
    pub fn finalized(&self) -> bool {
        self.status != "recording"
    }

    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == FORMAT_VERSION,
            "Unsupported game history version"
        );
        ensure!(
            valid_id(&self.id) && valid_id(&self.recorder_id),
            "Invalid history identifier"
        );
        ensure!(
            ["recording", "finished", "left", "interrupted"].contains(&self.status.as_str()),
            "Invalid recording status"
        );
        ensure!(self.players.len() == 8, "A game must contain exactly P1–P8");
        ensure!(
            self.started_at.len() <= 40
                && self.updated_at.len() <= 40
                && self.world_key.len() <= 2048,
            "Invalid game metadata"
        );
        chrono::DateTime::parse_from_rfc3339(&self.started_at)
            .context("Invalid game start time")?;
        chrono::DateTime::parse_from_rfc3339(&self.updated_at).context("Invalid update time")?;
        if let Some(end) = &self.ended_at {
            chrono::DateTime::parse_from_rfc3339(end).context("Invalid game end time")?;
        }
        ensure!(
            self.finalized() == self.ended_at.is_some(),
            "Finalized status and end time disagree"
        );
        if let Some(id) = &self.local_steam_id {
            ensure!(valid_steam_id(id), "Invalid local SteamID64");
        }
        ensure!(
            self.setup.len() <= 128 && self.setup.keys().all(|key| key.len() <= 128),
            "Too many setup fields"
        );
        let mut steam_ids = BTreeSet::new();
        ensure!(
            self.map_name.as_ref().is_none_or(|name| name.len() <= 1024),
            "Map name is too long"
        );
        for (index, player) in self.players.iter().enumerate() {
            ensure!(
                player.slot == index + 1,
                "Player slots must be ordered P1–P8"
            );
            ensure!(
                ["human", "ai", "empty", "unknown"].contains(&player.kind.as_str()),
                "Invalid participant kind"
            );
            ensure!(
                player
                    .steam_name
                    .as_ref()
                    .is_none_or(|name| name.len() <= 2048),
                "Player name is too long"
            );
            if let Some(id) = &player.steam_id {
                ensure!(valid_steam_id(id), "Invalid SteamID64");
                ensure!(
                    steam_ids.insert(id),
                    "Steam account appears in multiple slots"
                );
            }
            ensure!(
                player
                    .result
                    .as_deref()
                    .is_none_or(|result| ["win", "loss"].contains(&result)),
                "Invalid player result"
            );
        }
        Ok(())
    }
}

pub fn valid_steam_id(value: &str) -> bool {
    value.parse::<u64>().ok().is_some_and(|id| {
        id >> 56 == 1 && (id >> 52) & 15 == 1 && id as u32 != 0 && id.to_string() == value
    })
}

fn valid_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}

pub fn new_id() -> Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes)
        .map_err(|error| anyhow::anyhow!("Cannot generate history ID: {error}"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Ok(format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    ))
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn directory() -> PathBuf {
    crate::config::config_path()
        .parent()
        .unwrap_or(Path::new("."))
        .join("history")
}

pub fn game_path(root: &Path, id: &str) -> Result<PathBuf> {
    ensure!(valid_id(id), "Invalid game ID");
    Ok(root.join("games").join(format!("{id}.json")))
}

/// A temporary replacement is only an implementation detail, never a second history record.
pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
        };
        let from: Vec<u16> = temporary.as_os_str().encode_wide().chain([0]).collect();
        let to: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        ensure!(
            unsafe {
                MoveFileExW(
                    from.as_ptr(),
                    to.as_ptr(),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )
            } != 0,
            "Cannot replace history JSON: {}",
            std::io::Error::last_os_error()
        );
    }
    #[cfg(not(windows))]
    fs::rename(temporary, path)?;
    Ok(())
}

pub fn write_record(root: &Path, record: &GameRecord) -> Result<()> {
    record.validate()?;
    let path = game_path(root, &record.id)?;
    if path.exists() {
        let existing = read_record(&path)?;
        if existing.finalized() {
            ensure!(
                existing == *record,
                "Finalized game {} is immutable",
                record.id
            );
            return Ok(());
        }
        ensure!(
            existing.recorder_id == record.recorder_id && existing.started_at == record.started_at,
            "Recording identity changed"
        );
    }
    atomic_json(&path, record)
}

pub fn read_record(path: &Path) -> Result<GameRecord> {
    ensure!(
        fs::metadata(path)?.len() <= MAX_RECORD_BYTES,
        "Game file is too large"
    );
    let record: GameRecord = serde_json::from_slice(&fs::read(path)?)?;
    record.validate()?;
    Ok(record)
}

pub fn load_games(root: &Path) -> Result<(Vec<GameRecord>, usize)> {
    let folder = root.join("games");
    if !folder.exists() {
        return Ok((Vec::new(), 0));
    }
    let mut games = Vec::new();
    let mut errors = 0;
    for entry in fs::read_dir(folder)? {
        let path = entry?.path();
        if path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        match read_record(&path) {
            Ok(game) => games.push(game),
            Err(_) => errors += 1,
        }
    }
    games.sort_by(|a, b| {
        b.started_at
            .cmp(&a.started_at)
            .then_with(|| a.id.cmp(&b.id))
    });
    Ok((games, errors))
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ObservedName {
    pub name: String,
    pub first_seen: String,
    pub last_seen: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct JournalPlayer {
    pub steam_id: String,
    pub latest_name: String,
    pub color: Option<i32>,
    pub first_seen: String,
    pub last_seen: String,
    pub names: Vec<ObservedName>,
    pub games: Vec<String>,
    pub local_games: usize,
    pub wins: usize,
    pub losses: usize,
    pub kills: u64,
    pub units_lost: u64,
    pub acquired_gold: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlayerJournal {
    pub schema_version: u32,
    pub rebuilt_at: String,
    pub players: Vec<JournalPlayer>,
}

pub fn build_journal(games: &[GameRecord], recorder_id: &str) -> PlayerJournal {
    let mut players = BTreeMap::<String, JournalPlayer>::new();
    let mut seen_games = BTreeSet::new();
    for game in games {
        if !seen_games.insert(&game.id) {
            continue;
        }
        let mut seen_accounts = BTreeSet::new();
        for member in &game.players {
            let Some(id) = member.steam_id.as_ref().filter(|id| valid_steam_id(id)) else {
                continue;
            };
            if !seen_accounts.insert(id) {
                continue;
            }
            let player = players.entry(id.clone()).or_insert_with(|| JournalPlayer {
                steam_id: id.clone(),
                first_seen: game.started_at.clone(),
                last_seen: game.started_at.clone(),
                ..Default::default()
            });
            player.games.push(game.id.clone());
            if game.recorder_id == recorder_id {
                player.local_games += 1;
            }
            if member.result.as_deref() == Some("win") {
                player.wins += 1;
            }
            if member.result.as_deref() == Some("loss") {
                player.losses += 1;
            }
            player.kills += statistic(member, "army_killed");
            player.units_lost += statistic(member, "army_lost");
            player.acquired_gold += statistic(member, "stat_gold_earned");
            player.first_seen = player.first_seen.clone().min(game.started_at.clone());
            if game.started_at >= player.last_seen {
                player.last_seen = game.started_at.clone();
                player.color = member.color;
                if let Some(name) = &member.steam_name {
                    player.latest_name = name.clone();
                }
            }
            if let Some(name) = member
                .steam_name
                .as_ref()
                .filter(|name| !name.trim().is_empty())
            {
                if let Some(observed) = player.names.iter_mut().find(|item| item.name == *name) {
                    observed.first_seen = observed.first_seen.clone().min(game.started_at.clone());
                    observed.last_seen = observed.last_seen.clone().max(game.started_at.clone());
                } else {
                    player.names.push(ObservedName {
                        name: name.clone(),
                        first_seen: game.started_at.clone(),
                        last_seen: game.started_at.clone(),
                    });
                }
            }
        }
    }
    let mut players: Vec<_> = players.into_values().collect();
    for player in &mut players {
        player.names.sort_by(|a, b| b.last_seen.cmp(&a.last_seen));
    }
    players.sort_by(|a, b| {
        b.last_seen
            .cmp(&a.last_seen)
            .then_with(|| a.steam_id.cmp(&b.steam_id))
    });
    PlayerJournal {
        schema_version: FORMAT_VERSION,
        rebuilt_at: now(),
        players,
    }
}

fn statistic(player: &Participant, name: &str) -> u64 {
    player
        .statistics
        .get(name)
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

pub fn search_player(player: &JournalPlayer, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || player.steam_id.contains(&query)
        || player
            .names
            .iter()
            .any(|name| name.name.to_lowercase().contains(&query))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Archive {
    pub schema_version: u32,
    pub exported_at: String,
    pub games: Vec<GameRecord>,
}

pub fn read_archive(path: &Path) -> Result<Archive> {
    ensure!(
        fs::metadata(path)?.len() <= MAX_ARCHIVE_BYTES,
        "Import exceeds 64 MB"
    );
    let archive: Archive = serde_json::from_slice(&fs::read(path)?)?;
    ensure!(
        archive.schema_version == FORMAT_VERSION && archive.games.len() <= 20_000,
        "Unsupported archive version or too many games"
    );
    let mut ids = BTreeSet::new();
    for game in &archive.games {
        game.validate()?;
        ensure!(
            serde_json::to_vec_pretty(game)?.len() as u64 <= MAX_RECORD_BYTES,
            "An imported game exceeds 4 MB"
        );
        ensure!(
            ids.insert(&game.id),
            "Archive contains duplicate record IDs"
        );
    }
    Ok(archive)
}

#[derive(Default, Debug, Clone)]
pub struct ImportPreview {
    pub new_games: usize,
    pub duplicates: usize,
    pub conflicts: usize,
}

pub fn preview_import(existing: &[GameRecord], archive: &Archive) -> ImportPreview {
    let mut result = ImportPreview::default();
    for incoming in &archive.games {
        match existing.iter().find(|game| game.id == incoming.id) {
            None => result.new_games += 1,
            Some(game) if game == incoming => result.duplicates += 1,
            Some(_) => result.conflicts += 1,
        }
    }
    result
}

pub fn import_new(root: &Path, archive: &Archive) -> Result<usize> {
    let mut imported = 0;
    for game in &archive.games {
        let path = game_path(root, &game.id)?;
        if path.exists() {
            continue;
        }
        game.validate()?;
        // A foreign unfinished capture is still the sender's immutable observation.
        atomic_json(&path, game)?;
        imported += 1;
    }
    Ok(imported)
}

enum WriteTask {
    Record(Box<GameRecord>),
    Journal(PlayerJournal),
    Flush(mpsc::Sender<()>),
    Stop,
}

pub struct HistoryWriter {
    sender: mpsc::Sender<WriteTask>,
    errors: mpsc::Receiver<String>,
    handle: Option<thread::JoinHandle<()>>,
    _lock: fs::File,
}

impl HistoryWriter {
    pub fn start(root: PathBuf) -> Result<Self> {
        fs::create_dir_all(root.join("games"))?;
        let lock_path = root.join(".writer.lock");
        let mut options = OpenOptions::new();
        options.create(true).truncate(false).read(true).write(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }
        let lock = options
            .open(lock_path)
            .context("History is open in another app instance")?;
        let (sender, tasks) = mpsc::channel();
        let (error_sender, errors) = mpsc::channel();
        let handle = thread::spawn(move || {
            while let Ok(task) = tasks.recv() {
                let result = match task {
                    WriteTask::Record(record) => write_record(&root, &record),
                    WriteTask::Journal(journal) => {
                        atomic_json(&root.join("players.json"), &journal)
                    }
                    WriteTask::Flush(reply) => {
                        let _ = reply.send(());
                        continue;
                    }
                    WriteTask::Stop => break,
                };
                if let Err(error) = result {
                    let _ = error_sender.send(format!("{error:#}"));
                }
            }
        });
        Ok(Self {
            sender,
            errors,
            handle: Some(handle),
            _lock: lock,
        })
    }
    pub fn record(&self, record: GameRecord) -> Result<()> {
        self.sender
            .send(WriteTask::Record(Box::new(record)))
            .context("History writer stopped")
    }
    pub fn journal(&self, journal: PlayerJournal) -> Result<()> {
        self.sender
            .send(WriteTask::Journal(journal))
            .context("History writer stopped")
    }
    pub fn flush(&self) -> Result<()> {
        let (reply, rx) = mpsc::channel();
        self.sender.send(WriteTask::Flush(reply))?;
        rx.recv_timeout(std::time::Duration::from_secs(5))
            .context("History save is still pending")?;
        Ok(())
    }
    pub fn error(&self) -> Option<String> {
        self.errors.try_iter().last()
    }
}

impl Drop for HistoryWriter {
    fn drop(&mut self) {
        let _ = self.sender.send(WriteTask::Stop);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

pub fn recorder_id(root: &Path) -> Result<String> {
    let path = root.join("recorder.json");
    if path.exists() {
        let value: Value = serde_json::from_slice(&fs::read(&path)?)?;
        let id = value
            .get("id")
            .and_then(Value::as_str)
            .context("Invalid recorder identifier")?;
        ensure!(valid_id(id), "Invalid recorder identifier");
        return Ok(id.into());
    }
    let id = new_id()?;
    atomic_json(&path, &serde_json::json!({"id":id}))?;
    Ok(id)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn record(id: &str, name: &str) -> GameRecord {
        GameRecord {
            schema_version: 1,
            id: id.into(),
            recorder_id: "22222222-2222-4222-8222-222222222222".into(),
            app_version: "test".into(),
            started_at: "2026-11-01T00:00:00.000Z".into(),
            updated_at: "2026-11-01T00:00:05.000Z".into(),
            ended_at: None,
            status: "recording".into(),
            finish_reason: None,
            process_id: 1,
            lobby_id: None,
            local_steam_id: Some("76561198027391269".into()),
            world_key: "test".into(),
            first_tick: 1,
            last_tick: 50,
            observed_seconds: 5,
            partial: false,
            map_name: Some("Map".into()),
            map_size: Some(400),
            setup: BTreeMap::new(),
            players: (1..=8)
                .map(|slot| Participant {
                    slot,
                    kind: if slot == 1 { "human" } else { "empty" }.into(),
                    steam_id: (slot == 1).then(|| "76561198027391269".into()),
                    steam_name: (slot == 1).then(|| name.into()),
                    color: Some(1),
                    team: None,
                    is_self: slot == 1,
                    is_host: slot == 1,
                    result: None,
                    statistics: serde_json::json!({"army_killed":12}),
                })
                .collect(),
        }
    }
    #[test]
    fn journal_matches_old_names_without_merging_same_named_accounts() {
        let mut first = record("11111111-1111-4111-8111-111111111111", "Old Name");
        first.status = "finished".into();
        first.ended_at = Some(first.updated_at.clone());
        let mut second = first.clone();
        second.id = "33333333-3333-4333-8333-333333333333".into();
        second.started_at = "2026-11-02T00:00:00.000Z".into();
        second.players[0].steam_name = Some("New Name".into());
        let mut third = second.clone();
        third.id = "44444444-4444-4444-8444-444444444444".into();
        third.players[0].steam_id = Some("76561198027391270".into());
        let journal = build_journal(&[first.clone(), first, second, third], "local");
        assert_eq!(journal.players.len(), 2);
        let player = journal
            .players
            .iter()
            .find(|p| p.steam_id == "76561198027391269")
            .unwrap();
        assert_eq!(player.games.len(), 2);
        assert_eq!(player.latest_name, "New Name");
        assert_eq!(player.kills, 24);
        assert!(search_player(player, "OLD NAME"));
    }
    #[test]
    fn one_file_updates_then_freezes_and_import_is_idempotent() -> Result<()> {
        let root = std::env::temp_dir().join(format!("shc-history-{}", new_id()?));
        let mut game = record("11111111-1111-4111-8111-111111111111", "Test");
        write_record(&root, &game)?;
        game.last_tick = 200;
        write_record(&root, &game)?;
        assert_eq!(fs::read_dir(root.join("games"))?.count(), 1);
        game.status = "finished".into();
        game.ended_at = Some(game.updated_at.clone());
        write_record(&root, &game)?;
        let path = game_path(&root, &game.id)?;
        let final_bytes = fs::read(&path)?;
        game.last_tick = 500;
        assert!(write_record(&root, &game).is_err());
        assert_eq!(fs::read(&path)?, final_bytes);
        let saved = read_record(&path)?;
        let archive = Archive {
            schema_version: 1,
            exported_at: now(),
            games: vec![saved.clone()],
        };
        assert_eq!(preview_import(&[saved], &archive).duplicates, 1);
        assert_eq!(import_new(&root, &archive)?, 0);
        fs::remove_file(path)?;
        fs::remove_dir(root.join("games"))?;
        fs::remove_dir(root)?;
        Ok(())
    }
    #[test]
    fn ids_cannot_escape_history_or_round_steam_ids() {
        assert!(game_path(Path::new("history"), "../escape").is_err());
        assert!(valid_steam_id("76561198027391269"));
        assert!(!valid_steam_id("0"));
        let ids: BTreeSet<_> = (0..1000).map(|_| new_id().unwrap()).collect();
        assert_eq!(ids.len(), 1000);
    }
}
