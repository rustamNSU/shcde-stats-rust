use std::collections::{BTreeMap, BTreeSet};

use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::addresses::{
    history_native_offsets as native, workspace_offsets as game, MAX_PLAYERS, PLAYER_STRIDE,
    PLAYER_TABLE_ANCHOR,
};
use super::memory::MemoryRead;

const MAX_MAP_SIZE: u32 = 800;
const MULTIPLAYER_GAME_TYPE: i32 = 3;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SessionParticipant {
    pub slot: usize,
    pub steam_id: Option<u64>,
    pub name: Option<String>,
    pub color: Option<i32>,
    pub is_ai: bool,
    pub is_self: bool,
    pub is_host: bool,
    pub team: Option<i32>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HistorySession {
    pub multiplayer_active: bool,
    pub director_multiplayer: bool,
    pub participants: Vec<SessionParticipant>,
    pub lobby_id: Option<u64>,
    pub local_steam_id: Option<u64>,
    pub map_name: Option<String>,
    pub map_size: Option<u32>,
    pub game_type: Option<i32>,
    pub game_token: Option<String>,
    pub setup: BTreeMap<String, Value>,
    /// Observed native states, not a declaration that the current match ended.
    pub results: BTreeMap<usize, String>,
    pub process_id: u32,
    pub world_loaded: bool,
    pub loading_from_save: bool,
    pub resyncing: bool,
    /// Confirmed mission-over transition belonging to the retained online world.
    pub completed: bool,
}

/// Adds a bounded native snapshot to the managed session without changing the game.
pub struct NativeHistoryReader<'a, R> {
    memory: &'a R,
    crusader_base: usize,
}

impl<'a, R: MemoryRead> NativeHistoryReader<'a, R> {
    pub fn new(memory: &'a R, crusader_base: usize) -> Self {
        Self {
            memory,
            crusader_base,
        }
    }

    /// Applies native fields only after every read succeeds. Callers must use
    /// managed lifecycle evidence before treating an observed result as final.
    pub fn enrich(&self, session: &mut HistorySession) -> Result<()> {
        ensure!(
            session.participants.len() <= MAX_PLAYERS,
            "Too many participants in the multiplayer session"
        );
        let mut slots = BTreeSet::new();
        let mut occupied_slots = BTreeSet::new();
        for participant in &session.participants {
            ensure!(
                (1..=MAX_PLAYERS).contains(&participant.slot) && slots.insert(participant.slot),
                "Invalid or repeated multiplayer player slot"
            );
            if participant.is_ai
                || participant.steam_id.is_some()
                || participant.name.is_some()
                || participant.color.is_some()
            {
                occupied_slots.insert(participant.slot);
            }
        }

        let game_type = self.read_i32(game::GAME_TYPE)?;
        ensure!(
            game_type == MULTIPLAYER_GAME_TYPE,
            "The native multiplayer world is not ready"
        );
        let map_size = self.read_u32(game::MAP_SIZE)?;
        ensure!(
            (1..=MAX_MAP_SIZE).contains(&map_size),
            "Invalid multiplayer map size"
        );
        let map_name = self.read_map_name()?;
        let starting_speed = self.read_option(native::STARTING_GAME_SPEED)?;
        ensure!(starting_speed > 0, "Multiplayer game setup is not ready");
        let peace_active = self.memory.read_u8(self.address(game::PEACE_ACTIVE)?)?;
        ensure!(peace_active <= 1, "Invalid multiplayer peace state");

        let setup = BTreeMap::from([
            ("starting_game_speed".into(), Value::from(starting_speed)),
            (
                "starting_goods_level".into(),
                Value::from(self.read_option(native::STARTING_GOODS_LEVEL)?),
            ),
            (
                "win_condition".into(),
                Value::from(self.read_option(native::WIN_CONDITION)?),
            ),
            (
                "fairness".into(),
                Value::from(self.read_option(native::FAIRNESS)?),
            ),
            (
                "peacetime_minutes".into(),
                Value::from(self.read_option(native::PEACETIME_MINUTES)?),
            ),
            ("peace_active".into(), Value::from(peace_active != 0)),
            (
                "peace_start_tick".into(),
                Value::from(self.read_u32(game::PEACE_START_TICK)?),
            ),
            (
                "peace_end_tick".into(),
                Value::from(self.read_u32(game::PEACE_END_TICK)?),
            ),
        ]);

        let mut teams = BTreeMap::new();
        let mut results = BTreeMap::new();
        for slot in slots {
            let team_offset = game::TEAMS
                .checked_add((slot - 1) * game::PLAYER_VALUE_STRIDE)
                .context("Multiplayer team offset overflow")?;
            teams.insert(slot, self.read_i32(team_offset)?);
            let result_offset = (slot - 1)
                .checked_mul(PLAYER_STRIDE)
                .and_then(|offset| offset.checked_add(PLAYER_TABLE_ANCHOR))
                .and_then(|offset| offset.checked_add(native::PLAYER_WIN_LOSS_FROM_GOLD))
                .context("Multiplayer player offset overflow")?;
            let result = match self.read_u32(result_offset)? {
                1 => Some("win"),
                2 => Some("loss"),
                _ => None,
            };
            if let Some(result) = result.filter(|_| occupied_slots.contains(&slot)) {
                results.insert(slot, result.into());
            }
        }

        // A loading transition must not combine a new roster with an old world.
        ensure!(
            self.read_i32(game::GAME_TYPE)? == game_type
                && self.read_u32(game::MAP_SIZE)? == map_size,
            "Multiplayer world changed during the history read"
        );

        session.map_name = map_name;
        session.map_size = Some(map_size);
        session.game_type = Some(game_type);
        session.setup.extend(setup);
        session.results = results;
        for participant in &mut session.participants {
            participant.team = teams.get(&participant.slot).copied();
        }
        Ok(())
    }

    fn read_map_name(&self) -> Result<Option<String>> {
        let length = self.read_i32(game::MAP_NAME_LENGTH)?;
        ensure!(
            length >= 0 && length as usize <= game::MAP_NAME_CAPACITY,
            "Invalid multiplayer map name length"
        );
        let base = self.address(game::MAP_NAME)?;
        let mut bytes = Vec::with_capacity(length as usize);
        for index in 0..length as usize {
            let address = base
                .checked_add(index)
                .context("Map name address overflow")?;
            let byte = self.memory.read_u8(address)?;
            if byte == 0 {
                break;
            }
            bytes.push(byte);
        }
        ensure!(
            self.read_i32(game::MAP_NAME_LENGTH)? == length,
            "Multiplayer map name changed during the history read"
        );
        let name = String::from_utf8_lossy(&bytes)
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>();
        let name = name.trim();
        Ok((!name.is_empty()).then(|| name.to_owned()))
    }

    fn read_option(&self, offset: usize) -> Result<i32> {
        self.read_i32(
            native::CHORE_MANAGER
                .checked_add(offset)
                .context("Multiplayer setup offset overflow")?,
        )
    }

    fn address(&self, offset: usize) -> Result<usize> {
        self.crusader_base
            .checked_add(offset)
            .context("Native history address overflow")
    }

    fn read_i32(&self, offset: usize) -> Result<i32> {
        self.memory.read_i32(self.address(offset)?)
    }

    fn read_u32(&self, offset: usize) -> Result<u32> {
        self.memory.read_u32(self.address(offset)?)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    const TEST_BASE: usize = 0x1000;

    #[derive(Default)]
    struct FakeMemory {
        bytes: BTreeMap<usize, u8>,
        changing_map_size: bool,
        size_reads: Cell<u32>,
    }

    impl FakeMemory {
        fn set(&mut self, offset: usize, bytes: &[u8]) {
            for (index, byte) in bytes.iter().copied().enumerate() {
                self.bytes.insert(TEST_BASE + offset + index, byte);
            }
        }

        fn int(&mut self, offset: usize, value: i32) {
            self.set(offset, &value.to_le_bytes());
        }

        fn option(&mut self, offset: usize, value: i32) {
            self.int(native::CHORE_MANAGER + offset, value);
        }

        fn name(&mut self, bytes: &[u8]) {
            self.int(game::MAP_NAME_LENGTH, bytes.len() as i32);
            self.set(game::MAP_NAME, bytes);
        }

        fn result(&mut self, slot: usize, state: i32) {
            self.int(
                PLAYER_TABLE_ANCHOR
                    + (slot - 1) * PLAYER_STRIDE
                    + native::PLAYER_WIN_LOSS_FROM_GOLD,
                state,
            );
        }

        fn read<const N: usize>(&self, address: usize) -> Result<[u8; N]> {
            let mut bytes = [0; N];
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = *self
                    .bytes
                    .get(&(address + index))
                    .context("Unmapped fixture memory")?;
            }
            Ok(bytes)
        }

        fn fixture() -> Self {
            let mut memory = Self::default();
            memory.int(game::GAME_TYPE, MULTIPLAYER_GAME_TYPE);
            memory.int(game::MAP_SIZE, 400);
            memory.name("Долина 🏰".as_bytes());
            memory.option(native::STARTING_GAME_SPEED, 50);
            memory.option(native::STARTING_GOODS_LEVEL, 2);
            memory.option(native::WIN_CONDITION, 1);
            memory.option(native::FAIRNESS, 0);
            memory.option(native::PEACETIME_MINUTES, 10);
            memory.set(game::PEACE_ACTIVE, &[1]);
            memory.int(game::PEACE_START_TICK, 30);
            memory.int(game::PEACE_END_TICK, 30_030);
            for slot in 1..=MAX_PLAYERS {
                memory.int(
                    game::TEAMS + (slot - 1) * game::PLAYER_VALUE_STRIDE,
                    slot as i32,
                );
                memory.result(slot, 0);
            }
            memory
        }
    }

    impl MemoryRead for FakeMemory {
        fn read_u8(&self, address: usize) -> Result<u8> {
            Ok(self.read::<1>(address)?[0])
        }
        fn read_u16(&self, address: usize) -> Result<u16> {
            Ok(u16::from_le_bytes(self.read(address)?))
        }
        fn read_i16(&self, address: usize) -> Result<i16> {
            Ok(i16::from_le_bytes(self.read(address)?))
        }
        fn read_u32(&self, address: usize) -> Result<u32> {
            if self.changing_map_size && address == TEST_BASE + game::MAP_SIZE {
                let reads = self.size_reads.get();
                self.size_reads.set(reads + 1);
                return Ok(if reads == 0 { 400 } else { 800 });
            }
            Ok(u32::from_le_bytes(self.read(address)?))
        }
        fn read_i32(&self, address: usize) -> Result<i32> {
            Ok(i32::from_le_bytes(self.read(address)?))
        }
    }

    fn session() -> HistorySession {
        HistorySession {
            multiplayer_active: true,
            world_loaded: true,
            participants: [1, 8]
                .map(|slot| SessionParticipant {
                    slot,
                    name: Some(format!("Player {slot}")),
                    ..SessionParticipant::default()
                })
                .to_vec(),
            ..HistorySession::default()
        }
    }

    #[test]
    fn reads_map_setup_and_one_based_teams_without_changing_managed_lifecycle() {
        let memory = FakeMemory::fixture();
        let mut session = session();
        session.loading_from_save = true;
        session.game_token = Some("managed-token".into());
        NativeHistoryReader::new(&memory, TEST_BASE)
            .enrich(&mut session)
            .expect("native history");
        assert_eq!(session.map_name.as_deref(), Some("Долина 🏰"));
        assert_eq!(session.map_size, Some(400));
        assert_eq!(session.game_type, Some(3));
        assert_eq!(session.participants[0].team, Some(1));
        assert_eq!(session.participants[1].team, Some(8));
        assert_eq!(session.setup["starting_game_speed"], 50);
        assert_eq!(session.setup["peacetime_minutes"], 10);
        assert_eq!(session.setup["peace_active"], true);
        assert_eq!(session.setup["peace_end_tick"], 30_030);
        assert!(session.world_loaded && session.multiplayer_active && session.loading_from_save);
        assert_eq!(session.game_token.as_deref(), Some("managed-token"));
    }

    #[test]
    fn raw_results_only_cover_current_participants_and_known_states() {
        let mut memory = FakeMemory::fixture();
        memory.result(1, 1);
        memory.result(2, 1);
        memory.result(8, 2);
        let mut session = session();
        session.participants.push(SessionParticipant {
            slot: 2,
            ..Default::default()
        });
        let reader = NativeHistoryReader::new(&memory, TEST_BASE);
        reader.enrich(&mut session).expect("results");
        assert_eq!(
            session.results,
            BTreeMap::from([(1, "win".into()), (8, "loss".into())])
        );

        memory.result(1, 0);
        memory.result(8, 3);
        NativeHistoryReader::new(&memory, TEST_BASE)
            .enrich(&mut session)
            .expect("ongoing or unknown state");
        assert!(session.results.is_empty());
    }

    #[test]
    fn malformed_names_and_unavailable_reads_do_not_partially_modify_session() {
        for length in [-1, game::MAP_NAME_CAPACITY as i32 + 1, i32::MAX] {
            let mut memory = FakeMemory::fixture();
            memory.int(game::MAP_NAME_LENGTH, length);
            let mut session = session();
            let before = session.clone();
            assert!(NativeHistoryReader::new(&memory, TEST_BASE)
                .enrich(&mut session)
                .is_err());
            assert_eq!(session, before);
        }
        let mut memory = FakeMemory::fixture();
        memory
            .bytes
            .remove(&(TEST_BASE + native::CHORE_MANAGER + native::FAIRNESS));
        let mut session = session();
        let before = session.clone();
        assert!(NativeHistoryReader::new(&memory, TEST_BASE)
            .enrich(&mut session)
            .is_err());
        assert_eq!(session, before);
    }

    #[test]
    fn name_decoding_handles_empty_full_and_non_utf8_values() {
        for (bytes, expected) in [
            (Vec::new(), None),
            (
                vec![b'A'; game::MAP_NAME_CAPACITY],
                Some("A".repeat(game::MAP_NAME_CAPACITY)),
            ),
            (b"Map\0stale".to_vec(), Some("Map".into())),
            (b"Map\xFF\n".to_vec(), Some("Map\u{FFFD}".into())),
        ] {
            let mut memory = FakeMemory::fixture();
            memory.name(&bytes);
            let mut session = session();
            NativeHistoryReader::new(&memory, TEST_BASE)
                .enrich(&mut session)
                .expect("bounded map name");
            assert_eq!(session.map_name, expected);
        }
    }

    #[test]
    fn rejects_invalid_slots_maps_modes_overflow_and_changing_worlds() {
        for slots in [vec![0], vec![9], vec![1, 1], vec![1; MAX_PLAYERS + 1]] {
            let memory = FakeMemory::fixture();
            let mut session = session();
            session.participants = slots
                .into_iter()
                .map(|slot| SessionParticipant {
                    slot,
                    ..SessionParticipant::default()
                })
                .collect();
            assert!(NativeHistoryReader::new(&memory, TEST_BASE)
                .enrich(&mut session)
                .is_err());
        }
        for (field, value) in [
            (game::MAP_SIZE, 0),
            (game::MAP_SIZE, 801),
            (game::GAME_TYPE, 2),
        ] {
            let mut memory = FakeMemory::fixture();
            memory.int(field, value);
            assert!(NativeHistoryReader::new(&memory, TEST_BASE)
                .enrich(&mut session())
                .is_err());
        }
        let mut memory = FakeMemory::fixture();
        memory.changing_map_size = true;
        let mut value = session();
        let before = value.clone();
        assert!(NativeHistoryReader::new(&memory, TEST_BASE)
            .enrich(&mut value)
            .is_err());
        assert_eq!(value, before);
        assert!(NativeHistoryReader::new(&memory, usize::MAX)
            .enrich(&mut session())
            .is_err());
    }
}
