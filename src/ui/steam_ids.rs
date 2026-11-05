use std::{cell::RefCell, rc::Rc, time::Duration};

use anyhow::{ensure, Context, Result};
use slint::{ComponentHandle, Model, Timer, TimerMode, VecModel};

use crate::{
    config::AppConfig,
    engine::{
        history_session::HistorySession,
        multiplayer::{MultiplayerPlayers, PlayerIdentity},
        service::LiveStatsSource,
    },
    AppWindow, SteamIdUiRow,
};

/// The SteamID view is separate from public statistics and never changes game data.
pub fn install(
    app: &AppWindow,
    source: Rc<RefCell<LiveStatsSource>>,
    config: Rc<RefCell<AppConfig>>,
) -> Timer {
    let cache = Rc::new(RefCell::new(IdentityCache::default()));
    app.on_refresh_steam_ids({
        let app = app.as_weak();
        let source = source.clone();
        let config = config.clone();
        let cache = cache.clone();
        move || {
            if let Some(app) = app.upgrade() {
                refresh(&app, &source, &config.borrow(), &mut cache.borrow_mut());
            }
        }
    });
    app.on_open_steam_profile({
        let app = app.as_weak();
        let config = config.clone();
        let cache = cache.clone();
        move |player_id, rendered_id| {
            let Some(app) = app.upgrade() else { return };
            let result = (|| -> Result<()> {
                ensure!(authorized(&app, &config.borrow()), "Admin key required");
                ensure!(app.get_settings_tab() == 3, "Open the SteamID tab first");
                let expected_id = rendered_id.parse::<u64>().context("Invalid SteamID64")?;
                let url = profile_for_player(&cache.borrow().players, player_id, expected_id)?;
                open_profile(&url)
            })();
            if let Err(error) = result {
                app.set_steam_id_status(format!("Profile was not opened: {error:#}").into());
            }
        }
    });
    refresh(app, &source, &config.borrow(), &mut cache.borrow_mut());
    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), {
        let app = app.as_weak();
        move || {
            if let Some(app) = app.upgrade() {
                refresh(&app, &source, &config.borrow(), &mut cache.borrow_mut());
            }
        }
    });
    timer
}

fn authorized(app: &AppWindow, config: &AppConfig) -> bool {
    admin_access(app.get_admin_mode(), config.admin_mode_enabled())
}

fn admin_access(ui_unlocked: bool, key_valid: bool) -> bool {
    ui_unlocked && key_valid
}

#[derive(Default)]
struct IdentityCache {
    players: MultiplayerPlayers,
    match_key: Option<(u32, String)>,
    between_matches: bool,
    resync_observed: bool,
    restore_attempted: bool,
}

impl IdentityCache {
    fn observe(&mut self, session: &HistorySession) -> bool {
        if session.resyncing {
            self.resync_observed = true;
            return false;
        }
        if !session.multiplayer_active || !session.director_multiplayer || session.completed {
            self.between_matches = true;
            self.resync_observed = false;
            return false;
        }
        if !session.world_loaded {
            return false;
        }
        let Some(token) = &session.game_token else {
            return false;
        };
        let current: std::collections::BTreeMap<_, _> = session
            .participants
            .iter()
            .filter(|member| {
                (1..=8).contains(&member.slot)
                    && !member.is_ai
                    && (member.steam_id.is_some() || member.name.is_some())
            })
            .map(|member| {
                (
                    member.slot,
                    PlayerIdentity {
                        name: member.name.clone(),
                        color: member.color,
                        steam_id: member.steam_id,
                    },
                )
            })
            .collect();
        if current.is_empty() {
            return false;
        }
        let key = (session.process_id, token.clone());
        let resynced_same_match = self.resync_observed
            && self
                .match_key
                .as_ref()
                .is_some_and(|previous| previous.0 == session.process_id)
            && current.iter().any(|(slot, player)| {
                player.steam_id.is_some()
                    && self
                        .players
                        .players
                        .get(slot)
                        .is_some_and(|old| old.steam_id == player.steam_id)
            })
            && current.iter().all(|(slot, player)| {
                self.players.players.get(slot).is_none_or(|old| {
                    old.steam_id.is_none()
                        || player.steam_id.is_none()
                        || old.steam_id == player.steam_id
                })
            });
        if self.between_matches || (self.match_key.as_ref() != Some(&key) && !resynced_same_match) {
            self.players.players.clear();
        }
        // Keep departed players within this match, but never carry slots into a new one.
        for (slot, mut player) in current {
            if let Some(previous) = self.players.players.get(&slot) {
                if player.steam_id.is_none() || player.steam_id == previous.steam_id {
                    player.name = player.name.or_else(|| previous.name.clone());
                    player.color = player.color.or(previous.color);
                    player.steam_id = player.steam_id.or(previous.steam_id);
                }
            }
            self.players.players.insert(slot, player);
        }
        self.match_key = Some(key);
        self.between_matches = false;
        self.resync_observed = false;
        true
    }

    fn restore_last_match(&mut self) {
        if self.restore_attempted {
            return;
        }
        self.restore_attempted = true;
        // Use only this installation's captures, never the newest imported friend's match.
        let root = crate::history::directory();
        let local_id = std::fs::read(root.join("recorder.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|value| {
                value
                    .get("id")
                    .and_then(|id| id.as_str())
                    .map(str::to_owned)
            });
        let Some(local_id) = local_id else { return };
        let Ok((games, _)) = crate::history::load_games(&root) else {
            return;
        };
        self.restore_local_game(&games, &local_id);
    }

    fn restore_local_game(&mut self, games: &[crate::history::GameRecord], local_id: &str) {
        if let Some(game) = games.iter().find(|game| game.recorder_id == local_id) {
            self.restore_game(game);
        }
    }

    fn restore_game(&mut self, game: &crate::history::GameRecord) {
        self.players.players = game
            .players
            .iter()
            .filter(|player| player.kind == "human")
            .map(|player| {
                (
                    player.slot,
                    PlayerIdentity {
                        name: player.steam_name.clone(),
                        color: player.color,
                        steam_id: player.steam_id.as_ref().and_then(|id| id.parse().ok()),
                    },
                )
            })
            .collect();
        let prefix = format!("{}:", game.process_id);
        let token = game
            .world_key
            .strip_prefix(&prefix)
            .unwrap_or(&game.world_key);
        self.match_key = Some((game.process_id, token.into()));
        self.between_matches = game.finalized();
    }
}

fn refresh(
    app: &AppWindow,
    source: &RefCell<LiveStatsSource>,
    config: &AppConfig,
    cache: &mut IdentityCache,
) {
    if !authorized(app, config) {
        set_rows(app, Vec::new());
        app.set_steam_id_status("Admin key required.".into());
        if app.get_settings_tab() == 3 {
            app.set_settings_tab(0);
        }
        return;
    }
    cache.restore_last_match();
    // Capture while other tabs are open so the first post-match visit also has identities.
    let session = source.borrow_mut().managed_history_session();
    let live = session.as_ref().is_ok_and(|session| cache.observe(session));
    let rows = to_rows(&cache.players, &crate::player_colors(app));
    if !rows.is_empty() {
        let available = rows
            .iter()
            .filter(|row| !row.profile_url.is_empty())
            .count();
        let count = rows.len();
        set_rows(app, rows);
        app.set_steam_id_status(if live {
                format!("{count} players · {available} Steam profiles available · refreshes every second.").into()
            } else {
                format!("Last multiplayer match · {count} players · {available} Steam profiles available · kept until a new match starts.").into()
            });
    } else {
        app.set_steam_id_status(match session {
            Err(error) => format!("Multiplayer identities unavailable: {error:#}").into(),
            Ok(_) => "Waiting for a multiplayer match. Its players will remain here after it ends."
                .into(),
        });
    }
}

fn set_rows(app: &AppWindow, rows: Vec<SteamIdUiRow>) {
    let current = app.get_steam_id_rows();
    if let Some(model) = current.as_any().downcast_ref::<VecModel<SteamIdUiRow>>() {
        if model.row_count() == rows.len() {
            for (index, row) in rows.into_iter().enumerate() {
                if model.row_data(index).as_ref() != Some(&row) {
                    model.set_row_data(index, row);
                }
            }
            return;
        }
    }
    app.set_steam_id_rows(Rc::new(VecModel::from(rows)).into());
}

pub fn to_rows(players: &MultiplayerPlayers, fallback_colors: &[i32; 8]) -> Vec<SteamIdUiRow> {
    players
        .players
        .iter()
        .filter(|(id, _)| (1..=8).contains(*id))
        .map(|(id, player)| {
            let name = player
                .name
                .as_deref()
                .filter(|name| !name.trim().is_empty())
                .map(str::to_owned)
                .unwrap_or_else(|| format!("Player {id}"));
            let steam_id = player.steam_id.filter(|id| *id != 0);
            SteamIdUiRow {
                player_id: *id as i32,
                name: name.into(),
                color_index: player
                    .color
                    .filter(|color| (1..=8).contains(color))
                    .unwrap_or(fallback_colors[id - 1]),
                steam_id: steam_id
                    .map(|id| id.to_string())
                    .unwrap_or_else(|| "Unavailable".into())
                    .into(),
                profile_url: steam_id.and_then(profile_url).unwrap_or_default().into(),
            }
        })
        .collect()
}

fn profile_url(steam_id: u64) -> Option<String> {
    let universe = steam_id >> 56;
    let account_type = (steam_id >> 52) & 0xf;
    let account_id = steam_id as u32;
    // Public individual accounts have a profile; lobby/group identifiers do not.
    (universe == 1 && account_type == 1 && account_id != 0)
        .then(|| format!("https://steamcommunity.com/profiles/{steam_id}/"))
}

fn profile_for_player(
    players: &MultiplayerPlayers,
    player_id: i32,
    expected_id: u64,
) -> Result<String> {
    ensure!((1..=8).contains(&player_id), "Invalid player slot");
    let player = players
        .players
        .get(&(player_id as usize))
        .context("Player is not in the displayed match")?;
    ensure!(
        player.steam_id == Some(expected_id),
        "Player identity changed; refresh the table"
    );
    profile_url(expected_id).context("Steam profile is unavailable for this identifier")
}

#[cfg(windows)]
pub(super) fn open_profile(url: &str) -> Result<()> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let operation: Vec<u16> = "open".encode_utf16().chain([0]).collect();
    let url: Vec<u16> = url.encode_utf16().chain([0]).collect();
    let result = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            operation.as_ptr(),
            url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    ensure!(
        (result as isize) > 32,
        "Windows could not open the Steam profile"
    );
    Ok(())
}

#[cfg(not(windows))]
pub(super) fn open_profile(_url: &str) -> Result<()> {
    anyhow::bail!("Steam profile links currently require Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::multiplayer::PlayerIdentity;

    const ACCOUNT: u64 = 76_561_198_027_391_269;

    fn session(token: &str, accounts: &[(usize, u64)]) -> HistorySession {
        HistorySession {
            multiplayer_active: true,
            director_multiplayer: true,
            world_loaded: true,
            game_token: Some(token.into()),
            process_id: 10,
            participants: accounts
                .iter()
                .map(
                    |&(slot, id)| crate::engine::history_session::SessionParticipant {
                        slot,
                        steam_id: Some(id),
                        name: Some(format!("Account {id}")),
                        color: Some(slot as i32),
                        ..Default::default()
                    },
                )
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn departed_players_end_empty_and_lobby_keep_the_last_match_and_links() -> Result<()> {
        let mut cache = IdentityCache::default();
        let mut active = session("first", &[(1, ACCOUNT), (4, ACCOUNT + 1)]);
        assert!(cache.observe(&active));
        let expected = to_rows(&cache.players, &[8; 8]);
        active.participants.pop();
        assert!(cache.observe(&active));
        assert_eq!(to_rows(&cache.players, &[8; 8]), expected);

        active.participants.clear();
        assert!(!cache.observe(&active));
        let mut ended = active.clone();
        ended.multiplayer_active = false;
        assert!(!cache.observe(&ended));
        let mut lobby = session("next", &[(2, ACCOUNT + 2)]);
        lobby.world_loaded = false;
        assert!(!cache.observe(&lobby));
        assert_eq!(to_rows(&cache.players, &[8; 8]), expected);
        assert_eq!(
            profile_for_player(&cache.players, 4, ACCOUNT + 1)?,
            format!("https://steamcommunity.com/profiles/{}/", ACCOUNT + 1)
        );
        Ok(())
    }

    #[test]
    fn next_loaded_match_replaces_old_slots_and_rejects_old_profile_clicks() {
        let mut cache = IdentityCache::default();
        assert!(cache.observe(&session("first", &[(1, ACCOUNT), (4, ACCOUNT + 1)])));
        assert!(cache.observe(&session("next", &[(1, ACCOUNT + 2)])));
        assert_eq!(cache.players.players.len(), 1);
        assert!(profile_for_player(&cache.players, 1, ACCOUNT).is_err());
        assert!(profile_for_player(&cache.players, 4, ACCOUNT + 1).is_err());
        assert!(profile_for_player(&cache.players, 1, ACCOUNT + 2).is_ok());

        assert!(!cache.observe(&HistorySession::default()));
        // The game can reuse the same managed roster pointer for its next round.
        assert!(cache.observe(&session("next", &[(2, ACCOUNT + 3)])));
        assert_eq!(cache.players.players.len(), 1);
        assert!(!cache.players.players.contains_key(&1));
    }

    #[test]
    fn loading_and_resync_do_not_replace_a_match_but_a_restarted_game_does() {
        let mut cache = IdentityCache::default();
        let first = session("first", &[(1, ACCOUNT), (4, ACCOUNT + 1)]);
        assert!(cache.observe(&first));
        let expected = to_rows(&cache.players, &[8; 8]);
        let mut loading = session("next", &[(2, ACCOUNT + 2)]);
        loading.resyncing = true;
        assert!(!cache.observe(&loading));
        assert_eq!(to_rows(&cache.players, &[8; 8]), expected);
        let mut restarted = session("first", &[(2, ACCOUNT + 2)]);
        restarted.process_id = 11;
        assert!(cache.observe(&restarted));
        assert_eq!(cache.players.players.len(), 1);
        assert!(!cache.players.players.contains_key(&1));
    }

    #[test]
    fn last_record_restores_exact_identity_and_profile_after_monitor_restart() -> Result<()> {
        let mut game = crate::history::tests::record(
            "11111111-1111-4111-8111-111111111111",
            "Last match name",
        );
        game.status = "finished".into();
        game.ended_at = Some(game.updated_at.clone());
        let mut imported = game.clone();
        imported.recorder_id = "33333333-3333-4333-8333-333333333333".into();
        imported.players[0].steam_name = Some("Imported other match".into());
        let mut cache = IdentityCache::default();
        cache.restore_local_game(&[imported, game.clone()], &game.recorder_id);
        assert!(!cache.observe(&HistorySession::default()));
        let rows = to_rows(&cache.players, &[8; 8]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "Last match name");
        assert_eq!(rows[0].steam_id, ACCOUNT.to_string());
        assert!(profile_for_player(&cache.players, 1, ACCOUNT)?.ends_with(&format!("/{ACCOUNT}/")));
        Ok(())
    }

    #[test]
    fn resync_and_monitor_restart_keep_departed_players_in_the_same_match() {
        let mut game =
            crate::history::tests::record("11111111-1111-4111-8111-111111111111", "Original name");
        game.process_id = 10;
        game.world_key = "10:first".into();
        let mut cache = IdentityCache::default();
        cache.restore_game(&game);
        assert!(cache.observe(&session("first", &[(4, ACCOUNT + 1)])));
        assert_eq!(cache.players.players.len(), 2);
        let resync = HistorySession {
            resyncing: true,
            ..Default::default()
        };
        assert!(!cache.observe(&resync));
        assert!(cache.observe(&session("replaced-list", &[(4, ACCOUNT + 1)])));
        assert_eq!(cache.players.players.len(), 2);
        assert!(profile_for_player(&cache.players, 1, ACCOUNT).is_ok());
    }

    #[test]
    fn admin_access_needs_both_valid_key_and_unlocked_ui() {
        assert!(admin_access(true, true));
        for pair in [(false, false), (true, false), (false, true)] {
            assert!(!admin_access(pair.0, pair.1));
        }
    }
    #[test]
    fn rows_use_real_slots_unicode_colors_and_exact_64bit_ids() {
        let mut players = MultiplayerPlayers::default();
        players.players.insert(
            4,
            PlayerIdentity {
                name: Some("Рустам 🏹".into()),
                color: Some(7),
                steam_id: Some(ACCOUNT),
            },
        );
        players.players.insert(
            1,
            PlayerIdentity {
                name: None,
                color: None,
                steam_id: None,
            },
        );
        let rows = to_rows(&players, &[8; 8]);
        assert_eq!(rows[0].player_id, 1);
        assert_eq!(rows[0].name, "Player 1");
        assert_eq!(rows[0].color_index, 8);
        assert_eq!(rows[0].steam_id, "Unavailable");
        assert!(rows[0].profile_url.is_empty());
        assert_eq!(rows[1].name, "Рустам 🏹");
        assert_eq!(rows[1].color_index, 7);
        assert_eq!(rows[1].steam_id, "76561198027391269");
        assert_eq!(
            rows[1].profile_url,
            "https://steamcommunity.com/profiles/76561198027391269/"
        );
    }
    #[test]
    fn invalid_accounts_and_stale_player_links_are_rejected() {
        for id in [0, 1, u64::MAX, ACCOUNT ^ (6u64 << 52)] {
            assert!(profile_url(id).is_none());
        }
        let mut players = MultiplayerPlayers::default();
        players.players.insert(
            1,
            PlayerIdentity {
                steam_id: Some(ACCOUNT),
                ..Default::default()
            },
        );
        assert!(profile_for_player(&players, 1, ACCOUNT).is_ok());
        assert!(profile_for_player(&players, 1, ACCOUNT + 1).is_err());
        assert!(profile_for_player(&players, 2, ACCOUNT).is_err());
        assert!(profile_for_player(&players, 0, ACCOUNT).is_err());
    }
}
