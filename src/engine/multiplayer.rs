//! Read-only multiplayer appearance import using the game's existing managed member list.
use super::addresses::mono_offsets as layout;
use super::history_session::{HistorySession, SessionParticipant};
use anyhow::{bail, ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub trait Memory {
    fn read(&self, address: usize, length: usize) -> Result<Vec<u8>>;
}

#[derive(Clone, Debug, Default)]
pub struct PlayerIdentity {
    pub name: Option<String>,
    pub color: Option<i32>,
    pub steam_id: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct MultiplayerPlayers {
    pub players: BTreeMap<usize, PlayerIdentity>,
}

impl MultiplayerPlayers {
    /// Imported names fall back to numbered slots; unknown colors retain the manual choice.
    pub fn appearance(&self, current_colors: [i32; 8]) -> [(String, i32); 8] {
        std::array::from_fn(|index| {
            let player = self.players.get(&(index + 1));
            let name = player
                .and_then(|p| p.name.as_deref())
                .filter(|name| !name.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Player {}", index + 1));
            let color = player
                .and_then(|p| p.color)
                .filter(|c| (1..=8).contains(c))
                .unwrap_or(current_colors[index]);
            (name, color)
        })
    }
    pub fn name_count(&self) -> usize {
        self.players.values().filter(|p| p.name.is_some()).count()
    }
    pub fn color_count(&self) -> usize {
        self.players.values().filter(|p| p.color.is_some()).count()
    }
}

struct Reader<'a, M>(&'a M);
impl<M: Memory> Reader<'_, M> {
    fn bytes(&self, address: usize, length: usize) -> Result<Vec<u8>> {
        ensure!(
            (0x10000..0x8000_0000_0000).contains(&address),
            "Invalid Mono pointer"
        );
        ensure!(
            length <= 1024 * 1024
                && address
                    .checked_add(length)
                    .is_some_and(|end| end < 0x8000_0000_0000),
            "Invalid Mono read size"
        );
        let bytes = self.0.read(address, length)?;
        ensure!(bytes.len() == length, "Incomplete Mono read");
        Ok(bytes)
    }
    fn ptr(&self, address: usize) -> Result<usize> {
        Ok(self.u64(address)? as usize)
    }
    fn u64(&self, address: usize) -> Result<u64> {
        Ok(u64::from_le_bytes(
            self.bytes(address, 8)?.as_slice().try_into()?,
        ))
    }
    fn i32(&self, address: usize) -> Result<i32> {
        Ok(i32::from_le_bytes(
            self.bytes(address, 4)?.as_slice().try_into()?,
        ))
    }
    fn byte(&self, address: usize) -> Result<u8> {
        Ok(self.bytes(address, 1)?[0])
    }
    fn cstring(&self, address: usize) -> Result<String> {
        let mut output = Vec::new();
        while output.len() < 256 {
            let position = address
                .checked_add(output.len())
                .context("Mono string overflow")?;
            let count = 64.min(0x1000 - (position & 0xfff)).min(256 - output.len());
            let bytes = self.bytes(position, count)?;
            if let Some(end) = bytes.iter().position(|b| *b == 0) {
                output.extend_from_slice(&bytes[..end]);
                return String::from_utf8(output).context("Invalid Mono metadata name");
            }
            output.extend(bytes);
        }
        bail!("Unterminated Mono metadata name")
    }
    fn class(&self, object: usize) -> Result<usize> {
        self.ptr(self.ptr(object)?)
    }
    fn class_name(&self, class: usize) -> Result<(String, String)> {
        Ok((
            self.cstring(self.ptr(class + layout::CLASS_NAMESPACE)?)?,
            self.cstring(self.ptr(class + layout::CLASS_NAME)?)?,
        ))
    }
    fn fields(&self, class: usize) -> Result<BTreeMap<String, usize>> {
        let mut definition = class;
        if self.byte(class + layout::CLASS_KIND)? == 3 {
            definition = self.ptr(self.ptr(class + layout::CLASS_GENERIC)?)?;
        }
        ensure!(
            [1, 2].contains(&self.byte(definition + layout::CLASS_KIND)?),
            "Unsupported Mono class kind"
        );
        let count = self.i32(definition + layout::CLASS_FIELD_COUNT)?;
        // The shipped MainViewModel has more than two thousand UI backing fields.
        ensure!((0..=8192).contains(&count), "Invalid Mono field count");
        let start = self.ptr(class + layout::CLASS_FIELDS)?;
        let bytes = self.bytes(start, count as usize * layout::FIELD_SIZE)?;
        let mut fields = BTreeMap::new();
        for field in bytes.chunks_exact(layout::FIELD_SIZE) {
            let name = self.cstring(u64::from_le_bytes(
                field[layout::FIELD_NAME..layout::FIELD_NAME + 8].try_into()?,
            ) as usize)?;
            let offset = i32::from_le_bytes(
                field[layout::FIELD_OFFSET..layout::FIELD_OFFSET + 4].try_into()?,
            );
            // Negative offsets are thread-static/special fields, never ordinary object data.
            if (0..=1024 * 1024).contains(&offset) {
                fields.insert(name, offset as usize);
            }
        }
        Ok(fields)
    }
    fn string(&self, object: usize) -> Result<String> {
        if object == 0 {
            return Ok(String::new());
        }
        let (namespace, name) = self.class_name(self.class(object)?)?;
        ensure!(
            namespace == "System" && name == "String",
            "Player name is not a Mono string"
        );
        let count = self.i32(object + layout::STRING_LENGTH)?;
        ensure!((0..=256).contains(&count), "Invalid player name length");
        let bytes = self.bytes(object + layout::STRING_DATA, count as usize * 2)?;
        ensure!(
            self.i32(object + layout::STRING_LENGTH)? == count,
            "Player name changed during read"
        );
        let utf16: Vec<_> = bytes
            .chunks_exact(2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .collect();
        let text = String::from_utf16(&utf16).context("Invalid player-name UTF-16")?;
        Ok(text
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect::<String>()
            .trim()
            .to_owned())
    }
}

fn required(fields: &BTreeMap<String, usize>, name: &str) -> Result<usize> {
    fields
        .get(name)
        .copied()
        .with_context(|| format!("Mono field {name} is not initialized"))
}

pub struct Runtime {
    root_global: usize,
    root: usize,
    platform: usize,
    member: usize,
    instance_offset: usize,
    members_offset: usize,
    player_id_offset: usize,
    player_name_offset: usize,
    colour_offset: usize,
    ai_offset: usize,
    steam_id_offset: Option<usize>,
    history: Option<HistoryLayout>,
    terminal: Option<TerminalLayout>,
}

struct HistoryLayout {
    active_offset: usize,
    map_loaded_offset: usize,
    loading_from_save_offset: usize,
    resyncing_offset: usize,
    lobby_offset: Option<usize>,
    lobby_class: Option<usize>,
    lobby_id_offset: Option<usize>,
    self_offset: usize,
    host_offset: usize,
    director: usize,
    director_instance_offset: usize,
    director_multiplayer_offset: usize,
}

struct TerminalLayout {
    main_view_model: usize,
    main_view_model_instance_offset: usize,
    mission_over_offset: usize,
    mission_over_panel_offset: usize,
    mission_over_panel: usize,
    archived_scores_offset: usize,
    game_data: usize,
    scenario_offset: usize,
    scenario: usize,
    game_over_state_offset: usize,
    game_over_event_offset: usize,
}

impl Runtime {
    pub fn locate(memory: &impl Memory, base: usize, file: &[u8]) -> Result<Self> {
        let reader = Reader(memory);
        // This compatibility profile is verified against the shipped x64 Unity Mono.
        // Require all metadata-layout accessors to match before using internal offsets.
        let anchors: &[(usize, &[u8])] = &[
            (
                layout::ACCESSOR_0,
                &[0x48, 0x8b, 0x05, 0x69, 0xae, 0x6a, 0x00, 0xc3],
            ),
            (
                layout::ACCESSOR_1,
                &[0x48, 0x8b, 0x1d, 0x54, 0xad, 0x69, 0x00],
            ),
            (layout::ACCESSOR_2, &[0x48, 0x8b, 0x41, 0x30, 0xc3]),
            (layout::ACCESSOR_3, &[0x48, 0x8b, 0x7f, 0x48]),
            (layout::ACCESSOR_4, &[0x48, 0x8b, 0x41, 0x08, 0xc3]),
            (layout::ACCESSOR_5, &[0x8b, 0x43, 0x18]),
            (
                layout::ACCESSOR_6,
                &[0x48, 0x8b, 0xb7, 0xd0, 0x00, 0x00, 0x00],
            ),
            (
                layout::ACCESSOR_7,
                &[
                    0xf6, 0x41, 0x30, 0x04, 0x75, 0x03, 0x33, 0xc0, 0xc3, 0x48, 0x8b, 0x01, 0x48,
                    0x63, 0x40, 0x5c, 0x48, 0x8b, 0x44, 0xc1, 0x48, 0xc3,
                ],
            ),
            (layout::ACCESSOR_8, &[0x8b, 0x41, 0x10, 0xc3]),
            (layout::ACCESSOR_9, &[0x8b, 0x41, 0x18, 0xc3]),
        ];
        for &(rva, expected) in anchors {
            ensure!(
                file_slice(file, rva, expected.len())? == expected
                    && reader.bytes(base + rva, expected.len())? == expected,
                "Unsupported Mono runtime layout"
            );
        }
        Self::discover(
            memory,
            base.checked_add(layout::ROOT_GLOBAL)
                .context("Mono module overflow")?,
            base.checked_add(layout::ASSEMBLIES_GLOBAL)
                .context("Mono module overflow")?,
        )
    }

    fn discover(
        memory: &impl Memory,
        root_global: usize,
        assemblies_global: usize,
    ) -> Result<Self> {
        let reader = Reader(memory);
        let root = reader.ptr(root_global)?;
        let mut node = reader.ptr(assemblies_global)?;
        let mut image = None;
        let mut visited = std::collections::BTreeSet::new();
        while node != 0 {
            ensure!(
                visited.len() < 512 && visited.insert(node),
                "Invalid Mono assembly chain"
            );
            let assembly = reader.ptr(node)?;
            let candidate = reader.ptr(assembly + layout::ASSEMBLY_IMAGE)?;
            if matches!(
                reader
                    .cstring(reader.ptr(candidate + layout::IMAGE_NAME)?)?
                    .as_str(),
                "Assembly-CSharp" | "Assembly-CSharp.dll"
            ) {
                image = Some(candidate);
                break;
            }
            node = reader.ptr(node + 8)?;
        }
        let image = image.context("Game managed assembly is not loaded")?;
        let cache = image + layout::IMAGE_CACHE;
        let bucket_count = reader.i32(cache + layout::CACHE_COUNT)?;
        ensure!(
            (1..=65536).contains(&bucket_count),
            "Invalid Mono class cache size"
        );
        let buckets = reader.ptr(cache + layout::CACHE_BUCKETS)?;
        // The cache provides its own next-node accessor. Read its instructions; never call it.
        let next_accessor = reader.ptr(cache + layout::CACHE_NEXT_ACCESSOR)?;
        let next_code = reader.bytes(next_accessor, 8)?;
        ensure!(
            next_code == [0x48, 0x8d, 0x81, 0x08, 0x01, 0x00, 0x00, 0xc3],
            "Unsupported Mono class-cache links"
        );
        let mut platform = None;
        let mut member = None;
        let mut director = None;
        let mut lobby = None;
        let mut main_view_model = None;
        let mut mission_over_panel = None;
        let mut game_data = None;
        let mut scenario = None;
        let heads = reader.bytes(buckets, bucket_count as usize * 8)?;
        let mut visited = std::collections::BTreeSet::new();
        for bucket in heads.chunks_exact(8) {
            let mut class = u64::from_le_bytes(bucket.try_into()?) as usize;
            while class != 0 {
                ensure!(
                    visited.len() < 20000 && visited.insert(class),
                    "Invalid Mono class-cache chain"
                );
                let (namespace, name) = reader.class_name(class)?;
                if namespace.is_empty() && name == "Platform_Multiplayer" {
                    platform = Some(class);
                }
                if namespace.is_empty() && name == "MPGameMember" {
                    member = Some(class);
                }
                if namespace.is_empty() && name == "Director" {
                    director = Some(class);
                }
                if namespace.is_empty() && name == "MPLobby" {
                    lobby = Some(class);
                }
                if namespace == "CrusaderDE" && name == "MainViewModel" {
                    main_view_model = Some(class);
                }
                if namespace == "CrusaderDE" && name == "HUD_MissionOver" {
                    mission_over_panel = Some(class);
                }
                if namespace.is_empty() && name == "GameData" {
                    game_data = Some(class);
                }
                if namespace.is_empty() && name == "Scenarios" {
                    scenario = Some(class);
                }
                if platform.is_some()
                    && member.is_some()
                    && director.is_some()
                    && lobby.is_some()
                    && main_view_model.is_some()
                    && mission_over_panel.is_some()
                    && game_data.is_some()
                    && scenario.is_some()
                {
                    break;
                }
                class = reader.ptr(class + layout::CLASS_NEXT)?;
            }
            if platform.is_some()
                && member.is_some()
                && director.is_some()
                && lobby.is_some()
                && main_view_model.is_some()
                && mission_over_panel.is_some()
                && game_data.is_some()
                && scenario.is_some()
            {
                break;
            }
        }
        let platform = platform.context("Multiplayer class is not initialized yet")?;
        let member = member.context("Multiplayer member class is not initialized yet")?;
        ensure!(
            reader.ptr(platform + layout::CLASS_IMAGE)? == image
                && reader.ptr(member + layout::CLASS_IMAGE)? == image
                && reader.ptr(member + layout::CLASS_NESTED_PARENT)? == platform,
            "Unexpected multiplayer class identity"
        );
        let platform_fields = reader.fields(platform)?;
        let member_fields = reader.fields(member)?;
        let director_fields = director.map(|class| reader.fields(class)).transpose()?;
        let lobby_fields = lobby.map(|class| reader.fields(class)).transpose()?;
        let view_fields = main_view_model.and_then(|class| reader.fields(class).ok());
        let mission_fields = mission_over_panel.and_then(|class| reader.fields(class).ok());
        let game_fields = game_data.and_then(|class| reader.fields(class).ok());
        let scenario_fields = scenario.and_then(|class| reader.fields(class).ok());
        let history = (|| {
            let director = director?;
            if reader.ptr(director + layout::CLASS_IMAGE).ok()? != image {
                return None;
            }
            if let Some(lobby) = lobby {
                if reader.ptr(lobby + layout::CLASS_IMAGE).ok()? != image {
                    return None;
                }
            }
            Some(HistoryLayout {
                active_offset: *platform_fields.get("<MPGameActive>k__BackingField")?,
                map_loaded_offset: *platform_fields.get("mapLoaded")?,
                loading_from_save_offset: *platform_fields.get("loadingFromSave")?,
                resyncing_offset: *platform_fields.get("resyncing")?,
                lobby_offset: platform_fields.get("activeLobby").copied(),
                lobby_class: lobby,
                lobby_id_offset: lobby_fields
                    .as_ref()
                    .and_then(|fields| fields.get("id"))
                    .copied(),
                self_offset: *member_fields.get("isSelf")?,
                host_offset: *member_fields.get("isHost")?,
                director,
                director_instance_offset: *director_fields.as_ref()?.get("instance")?,
                director_multiplayer_offset: *director_fields.as_ref()?.get("multiplayerGame")?,
            })
        })();
        let terminal = (|| {
            let main_view_model = main_view_model?;
            let mission_over_panel = mission_over_panel?;
            let game_data = game_data?;
            let scenario = scenario?;
            if [main_view_model, mission_over_panel, game_data, scenario]
                .iter()
                .any(|class| reader.ptr(*class + layout::CLASS_IMAGE).ok() != Some(image))
                || reader.ptr(scenario + layout::CLASS_NESTED_PARENT).ok() != Some(game_data)
            {
                return None;
            }
            Some(TerminalLayout {
                main_view_model,
                main_view_model_instance_offset: *view_fields.as_ref()?.get("instance")?,
                mission_over_offset: *view_fields.as_ref()?.get("_show_HUD_MissionOver")?,
                mission_over_panel_offset: *view_fields.as_ref()?.get("HUDMissionOver")?,
                mission_over_panel,
                archived_scores_offset: *mission_fields.as_ref()?.get("SkirmishMasters")?,
                game_data,
                scenario_offset: *game_fields.as_ref()?.get("scenario")?,
                scenario,
                game_over_state_offset: *scenario_fields.as_ref()?.get("gameOverState")?,
                game_over_event_offset: *scenario_fields
                    .as_ref()?
                    .get("inGameoverSituationEvents")?,
            })
        })();
        Ok(Self {
            root_global,
            root,
            platform,
            member,
            instance_offset: required(&platform_fields, "instance")?,
            members_offset: required(&platform_fields, "gameMembers")?,
            player_id_offset: required(&member_fields, "playerID")?,
            player_name_offset: required(&member_fields, "playerName")?,
            colour_offset: required(&member_fields, "colourID")?,
            ai_offset: required(&member_fields, "skirmishAI")?,
            // Older managed layouts may omit this field; appearance import must
            // keep working even when account identities are unavailable.
            steam_id_offset: member_fields.get("steamID").copied(),
            history,
            terminal,
        })
    }

    fn statics(&self, reader: &Reader<'_, impl Memory>, class: usize) -> Result<usize> {
        ensure!(
            reader.ptr(self.root_global)? == self.root,
            "Mono domain changed"
        );
        let domain_id = reader.i32(self.root + layout::DOMAIN_ID)?;
        ensure!((0..=64).contains(&domain_id), "Invalid Mono domain ID");
        let info = reader.ptr(class + layout::CLASS_RUNTIME)?;
        let max_domain = u16::from_le_bytes(reader.bytes(info, 2)?.as_slice().try_into()?) as i32;
        ensure!(
            domain_id <= max_domain,
            "Multiplayer class has no domain vtable"
        );
        let vtable = reader.ptr(info + layout::RUNTIME_VTABLES + domain_id as usize * 8)?;
        ensure!(
            reader.ptr(vtable)? == class && reader.byte(vtable + layout::VTABLE_FLAGS)? & 4 != 0,
            "Multiplayer static data is not ready"
        );
        let slots = reader.i32(class + layout::CLASS_VTABLE_SIZE)?;
        ensure!((0..=65536).contains(&slots), "Invalid Mono vtable size");
        reader.ptr(vtable + layout::VTABLE_STATIC_SLOTS + slots as usize * 8)
    }

    pub fn players(&self, memory: &impl Memory) -> Result<MultiplayerPlayers> {
        let reader = Reader(memory);
        let statics = self.statics(&reader, self.platform)?;
        let instance = reader.ptr(statics + self.instance_offset)?;
        if instance == 0 {
            bail!("No multiplayer session. Start or load a multiplayer match first");
        }
        ensure!(
            reader.class(instance)? == self.platform,
            "Multiplayer singleton changed"
        );
        let list = reader.ptr(instance + self.members_offset)?;
        if list == 0 {
            bail!("No multiplayer player list. Start or load a multiplayer match first");
        }
        let participants = self.roster(&reader, instance, list, false)?;
        ensure!(
            reader.ptr(statics + self.instance_offset)? == instance
                && reader.ptr(self.root_global)? == self.root,
            "Multiplayer singleton changed during read"
        );
        Ok(MultiplayerPlayers {
            players: participants
                .into_iter()
                .filter(|member| !member.is_ai)
                .map(|member| {
                    (
                        member.slot,
                        PlayerIdentity {
                            name: member.name,
                            color: member.color,
                            steam_id: member.steam_id,
                        },
                    )
                })
                .collect(),
        })
    }

    /// Reads lifecycle flags even after the game clears its roster. No game APIs are called.
    pub fn history_session(&self, memory: &impl Memory) -> Result<HistorySession> {
        let fields = self
            .history
            .as_ref()
            .context("Multiplayer history fields are not initialized")?;
        let reader = Reader(memory);
        let statics = self.statics(&reader, self.platform)?;
        let active = reader.byte(statics + fields.active_offset)?;
        ensure!(active <= 1, "Invalid multiplayer active flag");
        let director_statics = self.statics(&reader, fields.director)?;
        let director = reader.ptr(director_statics + fields.director_instance_offset)?;
        let director_multiplayer = if director != 0 {
            ensure!(
                reader.class(director)? == fields.director,
                "Director singleton changed"
            );
            let value = reader.byte(director + fields.director_multiplayer_offset)?;
            ensure!(value <= 1, "Invalid multiplayer director flag");
            value != 0
        } else {
            false
        };
        let instance = reader.ptr(statics + self.instance_offset)?;
        let mut session = HistorySession {
            multiplayer_active: active != 0,
            director_multiplayer,
            participants: (1..=8)
                .map(|slot| SessionParticipant {
                    slot,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        if instance != 0 {
            ensure!(
                reader.class(instance)? == self.platform,
                "Multiplayer singleton changed"
            );
            let loaded = reader.byte(instance + fields.map_loaded_offset)?;
            let loading = reader.byte(instance + fields.loading_from_save_offset)?;
            let resyncing = reader.byte(instance + fields.resyncing_offset)?;
            ensure!(
                loaded <= 1 && loading <= 1 && resyncing <= 1,
                "Invalid multiplayer lifecycle flags"
            );
            session.loading_from_save = loading != 0;
            session.resyncing = resyncing != 0;
            let list = reader.ptr(instance + self.members_offset)?;
            if list != 0 {
                let members = self.roster(&reader, instance, list, true)?;
                session.world_loaded = loaded != 0 && !members.is_empty();
                // Opaque process-local roster lifetime, not a shared match identifier.
                let mut identity = Sha256::new();
                identity.update(b"shcde-managed-roster-v1");
                for pointer in [self.root, instance, list] {
                    identity.update((pointer as u64).to_le_bytes());
                }
                session.game_token = Some(format!("{:x}", identity.finalize()));
                for member in members {
                    let index = member.slot - 1;
                    session.participants[index] = member;
                }
            }
            let self_members: Vec<_> = session
                .participants
                .iter()
                .filter(|member| member.is_self)
                .collect();
            ensure!(
                self_members.len() <= 1,
                "Ambiguous local multiplayer identity"
            );
            session.local_steam_id = self_members.first().and_then(|member| member.steam_id);
            if let (Some(offset), Some(class), Some(id_offset)) = (
                fields.lobby_offset,
                fields.lobby_class,
                fields.lobby_id_offset,
            ) {
                let lobby = reader.ptr(instance + offset)?;
                if lobby != 0 {
                    ensure!(reader.class(lobby)? == class, "Multiplayer lobby changed");
                    let id = reader.u64(lobby + id_offset)?;
                    session.lobby_id = (id != 0).then_some(id);
                    ensure!(
                        reader.ptr(instance + offset)? == lobby
                            && reader.u64(lobby + id_offset)? == id,
                        "Multiplayer lobby changed during read"
                    );
                }
            }
            ensure!(
                reader.byte(instance + fields.map_loaded_offset)? == loaded
                    && reader.byte(instance + fields.loading_from_save_offset)? == loading
                    && reader.byte(instance + fields.resyncing_offset)? == resyncing
                    && reader.ptr(instance + self.members_offset)? == list,
                "Multiplayer lifecycle changed during read"
            );
        }
        ensure!(
            reader.ptr(statics + self.instance_offset)? == instance
                && reader.byte(statics + fields.active_offset)? == active
                && reader.ptr(director_statics + fields.director_instance_offset)? == director
                && (director == 0
                    || reader.byte(director + fields.director_multiplayer_offset)?
                        == u8::from(director_multiplayer))
                && reader.ptr(self.root_global)? == self.root,
            "Multiplayer session changed during read"
        );
        Ok(session)
    }

    pub fn mission_over_visible(&self, memory: &impl Memory) -> Result<bool> {
        let Some(fields) = self.terminal.as_ref() else {
            return Ok(false);
        };
        let reader = Reader(memory);
        let statics = self.statics(&reader, fields.main_view_model)?;
        let instance = reader.ptr(statics + fields.main_view_model_instance_offset)?;
        if instance == 0 {
            return Ok(false);
        }
        ensure!(
            reader.class(instance)? == fields.main_view_model,
            "Main view model changed"
        );
        let visible = reader.byte(instance + fields.mission_over_offset)?;
        ensure!(visible <= 1, "Invalid mission-over screen flag");
        if visible == 0 {
            return Ok(false);
        }
        let panel = reader.ptr(instance + fields.mission_over_panel_offset)?;
        ensure!(
            reader.class(panel)? == fields.mission_over_panel,
            "Mission-over panel changed"
        );
        let archive = reader.byte(panel + fields.archived_scores_offset)?;
        ensure!(archive <= 1, "Invalid archived-score flag");
        let game_statics = self.statics(&reader, fields.game_data)?;
        let scenario = reader.ptr(game_statics + fields.scenario_offset)?;
        ensure!(
            reader.class(scenario)? == fields.scenario,
            "Scenario changed"
        );
        let game_over = reader.i32(scenario + fields.game_over_state_offset)?;
        ensure!(game_over >= 0, "Invalid game-over state");
        let event = reader.byte(scenario + fields.game_over_event_offset)?;
        ensure!(event <= 1, "Invalid game-over event flag");
        ensure!(
            reader.ptr(statics + fields.main_view_model_instance_offset)? == instance
                && reader.byte(instance + fields.mission_over_offset)? == visible
                && reader.ptr(instance + fields.mission_over_panel_offset)? == panel
                && reader.byte(panel + fields.archived_scores_offset)? == archive
                && reader.ptr(game_statics + fields.scenario_offset)? == scenario
                && reader.i32(scenario + fields.game_over_state_offset)? == game_over
                && reader.byte(scenario + fields.game_over_event_offset)? == event
                && reader.ptr(self.root_global)? == self.root,
            "Mission-over screen changed during read"
        );
        Ok(archive == 0 && game_over > 0 && event == 1)
    }

    pub fn has_terminal_metadata(&self) -> bool {
        self.terminal.is_some()
    }

    fn roster(
        &self,
        reader: &Reader<'_, impl Memory>,
        instance: usize,
        list: usize,
        history: bool,
    ) -> Result<Vec<SessionParticipant>> {
        let list_class = reader.class(list)?;
        let (namespace, name) = reader.class_name(list_class)?;
        ensure!(
            namespace == "System.Collections.Generic" && name == "List`1",
            "Unexpected multiplayer list type"
        );
        let fields = reader.fields(list_class)?;
        let items_offset = required(&fields, "_items")?;
        let size_offset = required(&fields, "_size")?;
        let version_offset = required(&fields, "_version")?;
        let version = reader.i32(list + version_offset)?;
        let count = reader.i32(list + size_offset)?;
        ensure!(
            (0..=64).contains(&count),
            "Invalid multiplayer member count"
        );
        let array = reader.ptr(list + items_offset)?;
        let capacity = reader.ptr(array + layout::ARRAY_LENGTH)?;
        ensure!(
            count as usize <= capacity && capacity <= 256,
            "Invalid multiplayer member array"
        );
        let items = reader.bytes(array + layout::ARRAY_DATA, count as usize * 8)?;
        let mut players = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for item in items.chunks_exact(8) {
            let object = u64::from_le_bytes(item.try_into()?) as usize;
            ensure!(
                reader.class(object)? == self.member,
                "Multiplayer member changed"
            );
            let id = reader.i32(object + self.player_id_offset)?;
            if !(1..=8).contains(&id) {
                continue;
            }
            ensure!(ids.insert(id), "Ambiguous multiplayer player ID");
            let ai = reader.byte(object + self.ai_offset)?;
            ensure!(ai <= 1, "Invalid multiplayer AI flag");
            let colour = reader.i32(object + self.colour_offset)?;
            let name_object = reader.ptr(object + self.player_name_offset)?;
            let name = if ai == 0 || history {
                reader.string(name_object)?
            } else {
                String::new()
            };
            let steam_id_raw = if ai == 0 {
                self.steam_id_offset
                    .map(|offset| reader.u64(object + offset))
                    .transpose()?
            } else {
                None
            };
            let steam_id_unchanged = match (self.steam_id_offset, steam_id_raw) {
                (Some(offset), Some(id)) => reader.u64(object + offset)? == id,
                _ => true,
            };
            let flags = if history {
                let fields = self
                    .history
                    .as_ref()
                    .context("Multiplayer history fields are not initialized")?;
                let own = reader.byte(object + fields.self_offset)?;
                let host = reader.byte(object + fields.host_offset)?;
                ensure!(
                    own <= 1 && host <= 1,
                    "Invalid multiplayer participant flags"
                );
                ensure!(
                    reader.byte(object + fields.self_offset)? == own
                        && reader.byte(object + fields.host_offset)? == host,
                    "Multiplayer participant flags changed during read"
                );
                (own != 0, host != 0)
            } else {
                (false, false)
            };
            ensure!(
                reader.i32(object + self.player_id_offset)? == id
                    && reader.ptr(object + self.player_name_offset)? == name_object
                    && reader.byte(object + self.ai_offset)? == ai
                    && reader.i32(object + self.colour_offset)? == colour
                    && steam_id_unchanged,
                "Multiplayer identity changed during read"
            );
            players.push(SessionParticipant {
                slot: id as usize,
                name: (!name.is_empty()).then_some(name),
                color: (1..=8).contains(&colour).then_some(colour),
                steam_id: steam_id_raw.filter(|id| {
                    if history {
                        individual_account(*id)
                    } else {
                        *id != 0
                    }
                }),
                is_ai: ai != 0,
                is_self: flags.0,
                is_host: flags.1,
                team: None,
            });
        }
        ensure!(
            reader.i32(list + version_offset)? == version
                && reader.i32(list + size_offset)? == count
                && reader.ptr(list + items_offset)? == array
                && reader.ptr(instance + self.members_offset)? == list
                && reader.ptr(self.root_global)? == self.root,
            "Multiplayer list changed during read"
        );
        Ok(players)
    }
}

fn individual_account(id: u64) -> bool {
    let universe = id >> 56;
    let account_type = (id >> 52) & 0xf;
    let instance = (id >> 32) & 0xfffff;
    universe == 1 && account_type == 1 && instance <= 4 && id as u32 != 0
}

fn file_slice(bytes: &[u8], rva: usize, length: usize) -> Result<&[u8]> {
    fn word(bytes: &[u8], offset: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(
            bytes
                .get(offset..offset + 2)
                .context("Truncated Mono file")?
                .try_into()?,
        ))
    }
    fn dword(bytes: &[u8], offset: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(
            bytes
                .get(offset..offset + 4)
                .context("Truncated Mono file")?
                .try_into()?,
        ))
    }
    ensure!(bytes.starts_with(b"MZ"), "Invalid Mono file");
    let pe = dword(bytes, 0x3c)? as usize;
    ensure!(
        bytes.get(pe..pe + 4) == Some(b"PE\0\0"),
        "Invalid Mono PE header"
    );
    let count = word(bytes, pe + 6)? as usize;
    ensure!(count <= 96, "Invalid Mono section count");
    let start = pe + 24 + word(bytes, pe + 20)? as usize;
    for index in 0..count {
        let section = start + index * 40;
        let va = dword(bytes, section + 12)? as usize;
        let size = dword(bytes, section + 16)? as usize;
        let raw = dword(bytes, section + 20)? as usize;
        if rva >= va && rva.checked_add(length).is_some_and(|end| end <= va + size) {
            let at = raw + rva - va;
            return bytes.get(at..at + length).context("Truncated Mono section");
        }
    }
    bail!("Mono accessor has no file-backed section")
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    struct MemoryImage {
        bytes: Vec<u8>,
        changing_version: Option<usize>,
        version_reads: Cell<usize>,
        changing_steam_id: Option<usize>,
        steam_id_reads: Cell<u64>,
    }
    impl Memory for MemoryImage {
        fn read(&self, address: usize, length: usize) -> Result<Vec<u8>> {
            let at = address
                .checked_sub(0x10000)
                .context("Unmapped fixture address")?;
            let mut bytes = self
                .bytes
                .get(at..at + length)
                .context("Unmapped fixture read")?
                .to_vec();
            if self.changing_version == Some(address) && length == 4 {
                let count = self.version_reads.get();
                self.version_reads.set(count + 1);
                bytes.copy_from_slice(&(count as i32).to_le_bytes());
            }
            if self.changing_steam_id == Some(address) && length == 8 {
                let count = self.steam_id_reads.get();
                self.steam_id_reads.set(count + 1);
                bytes.copy_from_slice(&(0x0110_0001_0000_0001u64 + count).to_le_bytes());
            }
            Ok(bytes)
        }
    }
    struct Fixture {
        memory: MemoryImage,
        next: usize,
        root_global: usize,
        assemblies_global: usize,
        instance: usize,
        platform_statics: usize,
        director_instance: usize,
        lobby: usize,
        view_model: usize,
        mission_panel: usize,
        scenario: usize,
        list: usize,
        first_string: usize,
    }
    impl Fixture {
        fn allocate(&mut self, size: usize) -> usize {
            let address = self.next;
            self.next += size.next_multiple_of(16);
            address
        }
        fn put(&mut self, address: usize, bytes: &[u8]) {
            let at = address - 0x10000;
            self.memory.bytes[at..at + bytes.len()].copy_from_slice(bytes);
        }
        fn pointer(&mut self, address: usize, value: usize) {
            self.put(address, &(value as u64).to_le_bytes());
        }
        fn integer(&mut self, address: usize, value: i32) {
            self.put(address, &value.to_le_bytes());
        }
        fn cstring(&mut self, value: &str) -> usize {
            let address = self.allocate(256);
            self.put(address, value.as_bytes());
            address
        }
        fn class(&mut self, namespace: &str, name: &str, fields: &[(&str, usize)]) -> usize {
            let class = self.allocate(0x120);
            self.put(class + 0x1b, &[1]);
            let name = self.cstring(name);
            let namespace = self.cstring(namespace);
            self.pointer(class + 0x48, name);
            self.pointer(class + 0x50, namespace);
            self.integer(class + 0x100, fields.len() as i32);
            let storage = self.allocate(fields.len() * 0x20);
            self.pointer(class + 0x98, storage);
            for (index, (name, offset)) in fields.iter().enumerate() {
                let name = self.cstring(name);
                self.pointer(storage + index * 0x20 + 8, name);
                self.integer(storage + index * 0x20 + 0x18, *offset as i32);
            }
            class
        }
        fn object(&mut self, class: usize) -> usize {
            let object = self.allocate(0x100);
            let vtable = self.allocate(0x60);
            self.pointer(object, vtable);
            self.pointer(vtable, class);
            object
        }
        fn singleton(&mut self, class: usize) -> (usize, usize) {
            let statics = self.allocate(64);
            let vtable = self.allocate(0x60);
            self.pointer(vtable, class);
            self.put(vtable + 0x30, &[4]);
            self.pointer(vtable + 0x48, statics);
            let info = self.allocate(16);
            self.pointer(info + 8, vtable);
            self.pointer(class + 0xd0, info);
            let instance = self.object(class);
            self.pointer(statics + 16, instance);
            (statics, instance)
        }
        fn string(&mut self, class: usize, text: &str) -> usize {
            let object = self.object(class);
            let units: Vec<_> = text.encode_utf16().collect();
            self.integer(object + 0x10, units.len() as i32);
            self.put(
                object + 0x14,
                &units
                    .iter()
                    .flat_map(|u| u.to_le_bytes())
                    .collect::<Vec<_>>(),
            );
            object
        }
        fn new() -> Self {
            Self::with_steam_id_field(true)
        }
        fn with_steam_id_field(include_steam_id: bool) -> Self {
            let mut f = Self {
                memory: MemoryImage {
                    bytes: vec![0; 0x40000],
                    changing_version: None,
                    version_reads: Cell::new(0),
                    changing_steam_id: None,
                    steam_id_reads: Cell::new(0),
                },
                next: 0x11000,
                root_global: 0x10000,
                assemblies_global: 0x10008,
                instance: 0,
                platform_statics: 0,
                director_instance: 0,
                lobby: 0,
                view_model: 0,
                mission_panel: 0,
                scenario: 0,
                list: 0,
                first_string: 0,
            };
            let root = f.allocate(0x100);
            f.pointer(f.root_global, root);
            let assembly = f.allocate(0x100);
            let image = f.allocate(0x600);
            let image_name = f.cstring("Assembly-CSharp");
            f.pointer(image + 0x30, image_name);
            f.pointer(assembly + 0x60, image);
            let node = f.allocate(16);
            f.pointer(node, assembly);
            f.pointer(f.assemblies_global, node);
            let platform = f.class(
                "",
                "Platform_Multiplayer",
                &[
                    ("instance", 16),
                    ("gameMembers", 56),
                    ("<MPGameActive>k__BackingField", 40),
                    ("mapLoaded", 64),
                    ("loadingFromSave", 65),
                    ("resyncing", 66),
                    ("activeLobby", 72),
                ],
            );
            let mut member_fields = vec![
                ("playerID", 48),
                ("playerName", 32),
                ("colourID", 64),
                ("skirmishAI", 68),
                ("isSelf", 69),
                ("isHost", 70),
            ];
            if include_steam_id {
                member_fields.push(("steamID", 80));
            }
            let member = f.class("", "MPGameMember", &member_fields);
            let director = f.class("", "Director", &[("instance", 16), ("multiplayerGame", 48)]);
            let lobby = f.class("", "MPLobby", &[("id", 32)]);
            let view_model = f.class(
                "CrusaderDE",
                "MainViewModel",
                &[
                    ("instance", 16),
                    ("_show_HUD_MissionOver", 48),
                    ("HUDMissionOver", 56),
                ],
            );
            let panel = f.class("CrusaderDE", "HUD_MissionOver", &[("SkirmishMasters", 48)]);
            let game_data = f.class("", "GameData", &[("scenario", 16)]);
            let scenario = f.class(
                "",
                "Scenarios",
                &[("gameOverState", 48), ("inGameoverSituationEvents", 52)],
            );
            f.pointer(platform + 0x40, image);
            f.pointer(member + 0x40, image);
            f.pointer(director + 0x40, image);
            f.pointer(lobby + 0x40, image);
            for class in [view_model, panel, game_data, scenario] {
                f.pointer(class + 0x40, image);
            }
            f.pointer(scenario + 0x38, game_data);
            f.pointer(member + 0x38, platform);
            f.pointer(platform + 0x108, member);
            f.pointer(member + 0x108, director);
            f.pointer(director + 0x108, lobby);
            for (class, next) in [
                (lobby, view_model),
                (view_model, panel),
                (panel, game_data),
                (game_data, scenario),
            ] {
                f.pointer(class + 0x108, next);
            }
            let buckets = f.allocate(8);
            f.pointer(buckets, platform);
            f.integer(image + 0x4d0 + 0x18, 1);
            f.pointer(image + 0x4d0 + 0x20, buckets);
            let callback = f.allocate(16);
            f.put(callback, &[0x48, 0x8d, 0x81, 0x08, 0x01, 0x00, 0x00, 0xc3]);
            f.pointer(image + 0x4d0 + 0x10, callback);
            let statics = f.allocate(64);
            f.platform_statics = statics;
            let vtable = f.allocate(0x60);
            f.pointer(vtable, platform);
            f.put(vtable + 0x30, &[4]);
            f.pointer(vtable + 0x48, statics);
            let info = f.allocate(16);
            f.pointer(info + 8, vtable);
            f.pointer(platform + 0xd0, info);
            f.instance = f.object(platform);
            f.pointer(statics + 16, f.instance);
            let director_statics = f.allocate(64);
            let director_vtable = f.allocate(0x60);
            f.pointer(director_vtable, director);
            f.put(director_vtable + 0x30, &[4]);
            f.pointer(director_vtable + 0x48, director_statics);
            let director_info = f.allocate(16);
            f.pointer(director_info + 8, director_vtable);
            f.pointer(director + 0xd0, director_info);
            f.director_instance = f.object(director);
            f.pointer(director_statics + 16, f.director_instance);
            f.lobby = f.object(lobby);
            f.pointer(f.instance + 72, f.lobby);
            f.put(f.lobby + 32, &0x0100_0000_0000_0042u64.to_le_bytes());
            f.view_model = f.singleton(view_model).1;
            f.mission_panel = f.object(panel);
            f.pointer(f.view_model + 56, f.mission_panel);
            let game_statics = f.singleton(game_data).0;
            f.scenario = f.object(scenario);
            f.pointer(game_statics + 16, f.scenario);
            let list_class = f.class(
                "System.Collections.Generic",
                "List`1",
                &[("_items", 24), ("_size", 40), ("_version", 44)],
            );
            let definition = f.class(
                "System.Collections.Generic",
                "List`1",
                &[("_items", 0), ("_size", 0), ("_version", 0)],
            );
            let generic = f.allocate(16);
            f.pointer(generic, definition);
            f.pointer(list_class + 0xf0, generic);
            f.put(list_class + 0x1b, &[3]);
            f.list = f.object(list_class);
            f.pointer(f.instance + 56, f.list);
            let array = f.allocate(0x40);
            f.pointer(array + 0x18, 2);
            f.pointer(f.list + 24, array);
            f.integer(f.list + 40, 2);
            let strings = f.class("System", "String", &[]);
            for (index, id, name) in [(0, 4, "Alex 🏹"), (1, 1, "Рустам")] {
                let object = f.object(member);
                let name = f.string(strings, name);
                if index == 0 {
                    f.first_string = name;
                }
                f.integer(object + 48, id);
                f.integer(object + 64, if id == 4 { 2 } else { 7 });
                f.pointer(object + 32, name);
                if id == 1 {
                    f.put(object + 69, &[1]);
                    f.put(object + 70, &[1]);
                }
                f.put(
                    object + 80,
                    &(0x0110_0001_0000_0001u64 + index as u64 * 2).to_le_bytes(),
                );
                f.pointer(array + 0x20 + index * 8, object);
            }
            f
        }
        fn runtime(&self) -> Runtime {
            Runtime::discover(&self.memory, self.root_global, self.assemblies_global).unwrap()
        }
        fn member(&self, index: usize) -> usize {
            let array = Reader(&self.memory).ptr(self.list + 24).unwrap();
            Reader(&self.memory).ptr(array + 0x20 + index * 8).unwrap()
        }
        fn set_online(&mut self) {
            self.put(self.platform_statics + 40, &[1]);
            self.put(self.instance + 64, &[1]);
            self.put(self.director_instance + 48, &[1]);
        }
    }

    #[test]
    fn history_distinguishes_offline_from_online_and_maps_all_eight_slots() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        let offline = runtime.history_session(&f.memory).unwrap();
        assert!(
            !offline.multiplayer_active && !offline.director_multiplayer && !offline.world_loaded
        );
        f.set_online();
        let online = runtime.history_session(&f.memory).unwrap();
        assert!(online.multiplayer_active && online.director_multiplayer && online.world_loaded);
        assert_eq!(online.participants.len(), 8);
        assert_eq!(online.participants[0].slot, 1);
        assert_eq!(online.participants[0].name.as_deref(), Some("Рустам"));
        assert!(online.participants[0].is_self && online.participants[0].is_host);
        assert_eq!(online.local_steam_id, Some(0x0110_0001_0000_0003));
        assert_eq!(online.participants[3].name.as_deref(), Some("Alex 🏹"));
        assert!(online.participants[1].steam_id.is_none());
        assert!(online
            .game_token
            .as_ref()
            .is_some_and(|token| token.len() == 64));
        assert_eq!(
            online.game_token,
            runtime.history_session(&f.memory).unwrap().game_token
        );
    }

    #[test]
    fn history_returns_end_transition_when_roster_is_cleared() {
        let mut f = Fixture::new();
        f.set_online();
        let runtime = f.runtime();
        assert!(runtime.history_session(&f.memory).unwrap().world_loaded);
        f.pointer(f.instance + 56, 0);
        let ended = runtime.history_session(&f.memory).unwrap();
        assert!(ended.multiplayer_active && ended.director_multiplayer);
        assert!(!ended.world_loaded);
        assert!(ended.game_token.is_none());
        assert_eq!(ended.participants.len(), 8);
        assert!(ended
            .participants
            .iter()
            .all(|member| member.name.is_none() && member.steam_id.is_none()));
        assert!(runtime.players(&f.memory).is_err());
        f.put(f.platform_statics + 40, &[0]);
        assert!(
            !runtime
                .history_session(&f.memory)
                .unwrap()
                .multiplayer_active
        );
    }

    #[test]
    fn history_keeps_ai_separate_and_rejects_racing_identities() {
        let mut f = Fixture::new();
        f.set_online();
        let runtime = f.runtime();
        f.put(f.member(0) + 68, &[1]);
        f.put(f.instance + 65, &[1]);
        f.put(f.instance + 66, &[1]);
        let loaded = runtime.history_session(&f.memory).unwrap();
        assert!(loaded.loading_from_save && loaded.resyncing);
        assert!(loaded.participants[3].is_ai);
        assert!(loaded.participants[3].steam_id.is_none());
        assert_eq!(loaded.participants[3].name.as_deref(), Some("Alex 🏹"));
        f.memory.changing_steam_id = Some(f.member(1) + 80);
        assert!(runtime.history_session(&f.memory).is_err());
    }

    #[test]
    fn history_rejects_invalid_individual_accounts_without_breaking_name_import() {
        let mut f = Fixture::new();
        f.set_online();
        let runtime = f.runtime();
        f.put(f.member(0) + 80, &(u64::MAX - 2).to_le_bytes());
        let session = runtime.history_session(&f.memory).unwrap();
        assert!(session.participants[3].steam_id.is_none());
        assert_eq!(
            runtime.players(&f.memory).unwrap().players[&4].steam_id,
            Some(u64::MAX - 2)
        );
    }

    #[test]
    fn mission_completion_requires_real_end_event_and_excludes_archived_scores() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        assert!(!runtime.mission_over_visible(&f.memory).unwrap());
        f.put(f.view_model + 48, &[1]);
        assert!(!runtime.mission_over_visible(&f.memory).unwrap());
        f.integer(f.scenario + 48, 1);
        assert!(!runtime.mission_over_visible(&f.memory).unwrap());
        f.put(f.scenario + 52, &[1]);
        assert!(runtime.mission_over_visible(&f.memory).unwrap());
        f.put(f.mission_panel + 48, &[1]);
        assert!(!runtime.mission_over_visible(&f.memory).unwrap());
        f.put(f.mission_panel + 48, &[0]);
        f.integer(f.scenario + 48, 2);
        assert!(runtime.mission_over_visible(&f.memory).unwrap());
        f.put(f.scenario + 52, &[0]);
        assert!(!runtime.mission_over_visible(&f.memory).unwrap());
    }

    #[test]
    fn active_history_does_not_require_end_screen_classes_to_be_loaded() {
        let mut f = Fixture::new();
        f.set_online();
        let lobby_class = Reader(&f.memory).class(f.lobby).unwrap();
        f.pointer(lobby_class + layout::CLASS_NEXT, 0);
        let runtime = f.runtime();
        assert!(!runtime.has_terminal_metadata());
        assert!(runtime.history_session(&f.memory).unwrap().world_loaded);
        assert!(!runtime.mission_over_visible(&f.memory).unwrap());
    }

    #[test]
    fn terminal_discovery_handles_the_shipped_large_view_model_field_count() {
        let mut f = Fixture::new();
        f.set_online();
        f.memory.bytes.resize(2 * 1024 * 1024, 0);
        // The shipped MainViewModel has 2,162 fields, far more than a typical class.
        let names: Vec<_> = (0..2159).map(|index| format!("ui_field_{index}")).collect();
        let mut fields: Vec<_> = names.iter().map(|name| (name.as_str(), 128)).collect();
        fields.extend([
            ("instance", 16),
            ("_show_HUD_MissionOver", 48),
            ("HUDMissionOver", 56),
        ]);
        let large_class = f.class("CrusaderDE", "MainViewModel", &fields);
        let view_class = Reader(&f.memory).class(f.view_model).unwrap();
        let field_storage = Reader(&f.memory)
            .ptr(large_class + layout::CLASS_FIELDS)
            .unwrap();
        f.pointer(view_class + layout::CLASS_FIELDS, field_storage);
        f.integer(view_class + layout::CLASS_FIELD_COUNT, fields.len() as i32);
        let runtime = f.runtime();
        assert!(runtime.has_terminal_metadata());
        assert!(runtime.history_session(&f.memory).unwrap().world_loaded);

        // Invalid optional UI metadata cannot disable the existing appearance importer.
        f.integer(view_class + layout::CLASS_FIELD_COUNT, 8193);
        let runtime = f.runtime();
        assert!(!runtime.has_terminal_metadata());
        assert!(runtime.history_session(&f.memory).unwrap().world_loaded);
        assert_eq!(runtime.players(&f.memory).unwrap().name_count(), 2);
    }

    #[test]
    fn reads_runtime_offsets_unicode_and_real_ids_instead_of_list_order() {
        let f = Fixture::new();
        let names = f.runtime().players(&f.memory).unwrap();
        assert_eq!(names.players.keys().copied().collect::<Vec<_>>(), [1, 4]);
        assert_eq!(names.players[&1].name.as_deref(), Some("Рустам"));
        assert_eq!(names.players[&1].color, Some(7));
        assert_eq!(names.players[&4].name.as_deref(), Some("Alex 🏹"));
        assert_eq!(names.players[&4].color, Some(2));
        assert_eq!(names.players[&4].steam_id, Some(0x0110_0001_0000_0001));
        assert_eq!(names.players[&1].steam_id, Some(0x0110_0001_0000_0003));
    }

    #[test]
    fn keeps_steam_ids_as_unsigned_64_bit_values_without_float_rounding() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        let valid_id = 0x0110_0001_0000_0001u64;
        assert_ne!(valid_id, valid_id as f64 as u64);
        assert_eq!(
            runtime.players(&f.memory).unwrap().players[&4].steam_id,
            Some(valid_id)
        );
        f.put(f.member(0) + 80, &(u64::MAX - 2).to_le_bytes());
        assert_eq!(
            runtime.players(&f.memory).unwrap().players[&4].steam_id,
            Some(u64::MAX - 2)
        );
    }

    #[test]
    fn missing_or_zero_steam_id_keeps_appearance_available() {
        let mut f = Fixture::new();
        f.put(f.member(0) + 80, &0u64.to_le_bytes());
        let players = f.runtime().players(&f.memory).unwrap();
        assert_eq!(players.players[&4].steam_id, None);
        assert_eq!(players.players[&4].name.as_deref(), Some("Alex 🏹"));
        assert_eq!(players.players[&4].color, Some(2));

        let f = Fixture::with_steam_id_field(false);
        let runtime = f.runtime();
        assert_eq!(runtime.steam_id_offset, None);
        let players = runtime.players(&f.memory).unwrap();
        assert_eq!(players.players[&4].steam_id, None);
        assert_eq!(players.players[&1].steam_id, None);
        assert_eq!(players.appearance([8; 8])[0], ("Рустам".into(), 7));
    }

    #[test]
    fn ai_account_fields_are_not_read_and_changing_human_ids_are_rejected() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        let first = f.member(0);
        f.put(first + 68, &[1]);
        f.memory.changing_steam_id = Some(first + 80);
        let players = runtime.players(&f.memory).unwrap();
        assert!(!players.players.contains_key(&4));
        assert_eq!(f.memory.steam_id_reads.get(), 0);

        f.put(first + 68, &[0]);
        assert!(runtime
            .players(&f.memory)
            .unwrap_err()
            .to_string()
            .contains("identity changed"));
        assert_eq!(f.memory.steam_id_reads.get(), 2);
    }
    #[test]
    fn rejects_oversized_lists_and_strings_and_concurrent_list_changes() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        f.integer(f.list + 40, 1000);
        assert!(runtime.players(&f.memory).is_err());
        f.integer(f.list + 40, 2);
        f.integer(f.first_string + 0x10, 10000);
        assert!(runtime.players(&f.memory).is_err());
        f.integer(f.first_string + 0x10, 0);
        f.memory.changing_version = Some(f.list + 44);
        assert!(runtime.players(&f.memory).is_err());
    }

    #[test]
    fn missing_names_keep_numbered_slots_and_unknown_colors_keep_manual_colors() {
        let mut f = Fixture::new();
        f.integer(f.first_string + 0x10, 0);
        let array = Reader(&f.memory).ptr(f.list + 24).unwrap();
        let member = Reader(&f.memory).ptr(array + 0x20).unwrap();
        f.integer(member + 64, 99);
        let players = f.runtime().players(&f.memory).unwrap();
        let appearance = players.appearance([8; 8]);
        assert_eq!(appearance[3], ("Player 4".into(), 8));
        assert_eq!(appearance[0], ("Рустам".into(), 7));
        assert_eq!(appearance[1], ("Player 2".into(), 8));
    }

    #[test]
    fn ignores_ai_names_and_colors_and_rejects_stale_sessions() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        let array = Reader(&f.memory).ptr(f.list + 24).unwrap();
        let member = Reader(&f.memory).ptr(array + 0x20).unwrap();
        f.put(member + 68, &[1]);
        // AI strings need not even be valid: they are never decoded.
        f.pointer(member + 32, 1);
        let players = runtime.players(&f.memory).unwrap();
        assert_eq!(players.players.len(), 1);
        assert_eq!(players.appearance([8; 8])[3], ("Player 4".into(), 8));
        f.pointer(f.instance + 56, 0);
        assert!(runtime.players(&f.memory).is_err());
        f.pointer(f.root_global, 0);
        assert!(runtime
            .players(&f.memory)
            .unwrap_err()
            .to_string()
            .contains("domain changed"));
    }

    #[test]
    fn import_snapshot_does_not_change_after_memory_updates() {
        let mut f = Fixture::new();
        let runtime = f.runtime();
        let imported_players = runtime.players(&f.memory).unwrap();
        let imported = imported_players.appearance([8; 8]);
        f.integer(f.first_string + 0x10, 0);
        assert_eq!(imported[3].0, "Alex 🏹");
        let updated = runtime.players(&f.memory).unwrap();
        assert_eq!(updated.appearance([8; 8])[3].0, "Player 4");
        assert_eq!(
            updated.players[&4].steam_id,
            imported_players.players[&4].steam_id
        );
        assert_eq!(
            imported_players.players[&4].name.as_deref(),
            Some("Alex 🏹")
        );
    }
}
