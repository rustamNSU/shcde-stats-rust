use std::{
    cell::RefCell,
    collections::BTreeSet,
    rc::Rc,
    time::{Duration, Instant},
};

use anyhow::{ensure, Context, Result};
use slint::{ComponentHandle, Timer, TimerMode, VecModel};

use crate::{
    config::AppConfig,
    engine::service::LiveStatsSource,
    history::{self, recorder::Recorder, Archive, GameRecord, JournalPlayer, PlayerJournal},
    AppWindow, HistoryFieldUiRow, HistoryGameUiRow, HistoryRosterUiRow, JournalPlayerUiRow,
};

struct State {
    recorder: Option<Recorder>,
    games: Vec<GameRecord>,
    journal: PlayerJournal,
    pending: Option<Archive>,
    selected_game: Option<String>,
    selected_player: Option<String>,
    last_reload: Instant,
    catalog_dirty: bool,
    catalog_warning: Option<String>,
}

impl State {
    fn new() -> Self {
        Self {
            recorder: None,
            games: Vec::new(),
            journal: history::build_journal(&[], ""),
            pending: None,
            selected_game: None,
            selected_player: None,
            last_reload: Instant::now(),
            catalog_dirty: true,
            catalog_warning: None,
        }
    }
    fn ensure(&mut self) -> Result<()> {
        if self.recorder.is_none() {
            self.recorder = Some(Recorder::open(history::directory())?);
            self.reload()?;
        }
        Ok(())
    }
    fn reload(&mut self) -> Result<()> {
        if let Some(recorder) = &self.recorder {
            recorder.flush()?;
        }
        let (games, errors) = history::load_games(&history::directory())?;
        self.games = games;
        if let Some(active) = self.recorder.as_ref().and_then(Recorder::active).cloned() {
            self.upsert(active);
        }
        let id = self
            .recorder
            .as_ref()
            .map(Recorder::recorder_id)
            .unwrap_or("");
        self.journal = history::build_journal(&self.games, id);
        self.last_reload = Instant::now();
        self.catalog_dirty = false;
        self.catalog_warning =
            (errors != 0).then(|| format!("{errors} invalid files were preserved and skipped."));
        Ok(())
    }
    fn upsert(&mut self, record: GameRecord) {
        if let Some(existing) = self.games.iter_mut().find(|game| game.id == record.id) {
            *existing = record;
        } else {
            self.games.insert(0, record);
        }
    }
    fn sync_live(&mut self, previous_id: Option<&str>) -> Result<()> {
        let active = self.recorder.as_ref().and_then(Recorder::active).cloned();
        if let Some(id) =
            previous_id.filter(|id| active.as_ref().is_none_or(|record| record.id != *id))
        {
            if let Some(recorder) = &self.recorder {
                recorder.flush()?;
            }
            self.upsert(history::read_record(&history::game_path(
                &history::directory(),
                id,
            )?)?);
        }
        if let Some(record) = active {
            self.upsert(record);
        }
        Ok(())
    }
    fn update_journal_model(&mut self) {
        let id = self
            .recorder
            .as_ref()
            .map(Recorder::recorder_id)
            .unwrap_or("");
        self.journal = history::build_journal(&self.games, id);
        self.last_reload = Instant::now();
        self.catalog_dirty = false;
    }
}

pub struct Runtime {
    _timer: Timer,
    state: Rc<RefCell<State>>,
}

impl Runtime {
    pub fn finish(&self) -> Result<()> {
        if let Some(recorder) = self.state.borrow_mut().recorder.as_mut() {
            recorder.checkpoint()?;
        }
        Ok(())
    }
    pub fn finisher(&self) -> impl Fn() -> Result<()> {
        let state = self.state.clone();
        move || {
            if let Some(recorder) = state.borrow_mut().recorder.as_mut() {
                recorder.checkpoint()?;
            }
            Ok(())
        }
    }
}

#[derive(Clone)]
struct Handler {
    app: slint::Weak<AppWindow>,
    config: Rc<RefCell<AppConfig>>,
    state: Rc<RefCell<State>>,
}

impl Handler {
    fn run(&self, action: impl FnOnce(&AppWindow, &mut State) -> Result<()>) {
        let Some(app) = self.app.upgrade() else {
            return;
        };
        if !authorized(&app, &self.config.borrow()) {
            clear(&app);
            return;
        }
        let mut state = self.state.borrow_mut();
        if let Err(error) = state.ensure().and_then(|_| action(&app, &mut state)) {
            app.set_history_list_status(format!("{error:#}").into());
        }
    }
}

fn authorized(app: &AppWindow, config: &AppConfig) -> bool {
    app.get_admin_mode() && config.admin_mode_enabled()
}

fn clear(app: &AppWindow) {
    app.set_history_view(0);
    app.set_game_history_rows(Default::default());
    app.set_player_journal_rows(Default::default());
    app.set_history_roster_rows(Default::default());
    app.set_history_detail_fields(Default::default());
    app.set_history_player_games(Default::default());
    app.set_history_player_names(Default::default());
    app.set_history_detail_title("".into());
    app.set_history_detail_summary("".into());
    app.set_history_current_summary("".into());
    app.set_history_detail_steam_id("".into());
    app.set_history_import_pending(false);
    app.set_history_import_preview("".into());
    app.set_history_status("Admin key required.".into());
}

pub fn install(
    app: &AppWindow,
    source: Rc<RefCell<LiveStatsSource>>,
    config: Rc<RefCell<AppConfig>>,
) -> Runtime {
    let state = Rc::new(RefCell::new(State::new()));
    let handler = Handler {
        app: app.as_weak(),
        config: config.clone(),
        state: state.clone(),
    };
    app.on_history_show({
        let handler = handler.clone();
        move |view| {
            handler.run(|app, state| {
                ensure!((0..=2).contains(&view), "Invalid history view");
                app.set_history_view(view);
                app.set_history_search("".into());
                app.set_history_filter(0);
                if view != 0 {
                    state.reload()?;
                }
                render(app, state);
                Ok(())
            })
        }
    });
    app.on_history_search_changed({
        let handler = handler.clone();
        move |query| {
            handler.run(|app, state| {
                app.set_history_search(query);
                render(app, state);
                Ok(())
            })
        }
    });
    app.on_history_filter_changed({
        let handler = handler.clone();
        move |filter| {
            handler.run(|app, state| {
                app.set_history_filter(filter.clamp(0, 3));
                render(app, state);
                Ok(())
            })
        }
    });
    app.on_history_refresh({
        let handler = handler.clone();
        move || {
            handler.run(|app, state| {
                state.reload()?;
                render(app, state);
                Ok(())
            })
        }
    });
    app.on_history_open_game({
        let handler = handler.clone();
        move |id| {
            handler.run(|app, state| {
                ensure!(
                    state.games.iter().any(|game| game.id == id.as_str()),
                    "Game is unavailable; refresh history"
                );
                state.selected_game = Some(id.to_string());
                app.set_history_view(3);
                render(app, state);
                Ok(())
            })
        }
    });
    app.on_history_open_player({
        let handler = handler.clone();
        move |id| {
            handler.run(|app, state| {
                ensure!(
                    state
                        .journal
                        .players
                        .iter()
                        .any(|player| player.steam_id == id.as_str()),
                    "Player is unavailable; update the journal"
                );
                state.selected_player = Some(id.to_string());
                app.set_history_view(4);
                render(app, state);
                Ok(())
            })
        }
    });
    app.on_history_rebuild_players({let handler=handler.clone();move||handler.run(|app,state|{
        state.reload()?;
        let recorder=state.recorder.as_ref().context("Recorder unavailable")?;
        state.journal=recorder.rebuild_journal(&state.games)?;recorder.flush()?;
        render(app,state);
        app.set_history_list_status(format!("Updated players.json: {} Steam accounts from {} games. Search includes every recorded nickname.",state.journal.players.len(),state.games.len()).into());
        Ok(())
    })});
    app.on_history_open_folder({
        let handler = handler.clone();
        move || {
            handler.run(|_, _| {
                super::history_dialogs::open_folder(&history::directory().join("games"))
            })
        }
    });
    app.on_history_open_player_profile({
        let handler = handler.clone();
        move |id| {
            handler.run(|_, state| {
                ensure!(
                    state
                        .journal
                        .players
                        .iter()
                        .any(|player| player.steam_id == id.as_str()),
                    "Unknown journal player"
                );
                ensure!(history::valid_steam_id(id.as_str()), "Invalid SteamID64");
                super::steam_ids::open_profile(&format!(
                    "https://steamcommunity.com/profiles/{id}/"
                ))
            })
        }
    });
    app.on_history_export({
        let handler = handler.clone();
        move || {
            handler.run(|app, state| {
                state.reload()?;
                let Some(path) = super::history_dialogs::choose_json(true)? else {
                    return Ok(());
                };
                let selected = if app.get_history_view() == 1 {
                    filtered_games(app, state)
                } else if app.get_history_view() == 3 {
                    state
                        .games
                        .iter()
                        .filter(|game| Some(&game.id) == state.selected_game.as_ref())
                        .cloned()
                        .collect()
                } else if app.get_history_view() == 4 {
                    player_games(state, state.selected_player.as_deref().unwrap_or(""))
                        .into_iter()
                        .cloned()
                        .collect()
                } else {
                    state.games.clone()
                };
                let count = selected.len();
                // Exports never replace a canonical game or its player journal.
                let parent = path.parent().unwrap_or(std::path::Path::new("."));
                ensure!(
                    std::fs::canonicalize(parent)?
                        != std::fs::canonicalize(history::directory().join("games"))?,
                    "Choose an export location outside the games folder"
                );
                ensure!(
                    path.file_name().is_none_or(|name| !matches!(
                        name.to_string_lossy().to_ascii_lowercase().as_str(),
                        "config.json" | "players.json" | "recorder.json"
                    )),
                    "Choose a different export filename"
                );
                history::atomic_json(
                    &path,
                    &Archive {
                        schema_version: history::FORMAT_VERSION,
                        exported_at: history::now(),
                        games: selected,
                    },
                )?;
                app.set_history_list_status(
                    format!("Exported {count} game records to {}", path.display()).into(),
                );
                Ok(())
            })
        }
    });
    app.on_history_import({let handler=handler.clone();move||handler.run(|app,state|{
        let Some(path)=super::history_dialogs::choose_json(false)? else{return Ok(())};
        let archive=history::read_archive(&path)?;state.reload()?;
        let preview=history::preview_import(&state.games,&archive);
        app.set_history_import_preview(format!("{} new games · {} identical copies · {} conflicting IDs. Import adds new IDs only; existing game files stay unchanged.",preview.new_games,preview.duplicates,preview.conflicts).into());
        state.pending=Some(archive);app.set_history_import_pending(true);Ok(())
    })});
    app.on_history_confirm_import({let handler=handler.clone();move||handler.run(|app,state|{
        let archive=state.pending.take().context("Choose a history archive first")?;
        let imported=history::import_new(&history::directory(),&archive)?;
        app.set_history_import_pending(false);state.reload()?;
        let recorder=state.recorder.as_ref().context("Recorder unavailable")?;
        state.journal=recorder.rebuild_journal(&state.games)?;recorder.flush()?;
        render(app,state);app.set_history_list_status(format!("Imported {imported} new game records; existing files were preserved. Player journal updated.").into());Ok(())
    })});
    app.on_history_cancel_import({
        let handler = handler.clone();
        move || {
            handler.run(|app, state| {
                state.pending = None;
                app.set_history_import_pending(false);
                Ok(())
            })
        }
    });

    let timer = Timer::default();
    timer.start(TimerMode::Repeated, Duration::from_secs(1), {
        let handler = handler.clone();
        move || {
            let Some(app) = handler.app.upgrade() else {
                return;
            };
            if !authorized(&app, &handler.config.borrow()) {
                let mut state = handler.state.borrow_mut();
                clear(&app);
                if let Some(recorder) = state.recorder.as_mut() {
                    if let Err(error) = recorder.checkpoint() {
                        app.set_history_status(
                            format!("Final history save failed: {error:#}").into(),
                        );
                        return;
                    }
                }
                state.recorder = None;
                state.pending = None;
                state.games.clear();
                state.journal.players.clear();
                clear(&app);
                return;
            }
            handler.run(|app, state| {
                let previous_id = state
                    .recorder
                    .as_ref()
                    .and_then(|recorder| recorder.active())
                    .map(|record| record.id.clone());
                let sample = source.borrow_mut().history_sample();
                let recorder = state.recorder.as_mut().context("Recorder unavailable")?;
                match sample {
                    Ok(sample) => recorder.observe(&sample)?,
                    Err(error) => {
                        recorder.unavailable(source.borrow().history_process_running())?;
                        let message = format!(
                            "{} · Waiting for readable game data: {error:#}",
                            recorder.status()
                        );
                        if previous_id.is_some() && recorder.active().is_none() {
                            state.catalog_dirty = true;
                        }
                        state.sync_live(previous_id.as_deref())?;
                        if app.get_settings_tab() == 3 && state.catalog_dirty {
                            state.update_journal_model();
                            render(app, state);
                        }
                        app.set_history_status(message.into());
                        return Ok(());
                    }
                }
                app.set_history_status(recorder.status().into());
                let current_id = recorder.active().map(|record| record.id.clone());
                if previous_id != current_id {
                    state.catalog_dirty = true;
                }
                state.sync_live(previous_id.as_deref())?;
                if app.get_settings_tab() == 3
                    && (state.catalog_dirty
                        || state.last_reload.elapsed() >= Duration::from_secs(5))
                {
                    state.update_journal_model();
                    render(app, state);
                }
                Ok(())
            });
        }
    });
    if !authorized(app, &config.borrow()) {
        clear(app);
    } else {
        app.set_history_status("Waiting for a confirmed online multiplayer match.".into());
    }
    Runtime {
        _timer: timer,
        state,
    }
}

fn filtered_games(app: &AppWindow, state: &State) -> Vec<GameRecord> {
    let query = app.get_history_search().trim().to_lowercase();
    let matching_ids: BTreeSet<_> = state
        .journal
        .players
        .iter()
        .filter(|player| history::search_player(player, &query))
        .map(|player| player.steam_id.as_str())
        .collect();
    state
        .games
        .iter()
        .filter(|game| {
            let status = match app.get_history_filter() {
                1 => game.status == "finished",
                2 => game.status == "interrupted" || game.status == "left",
                3 => game.status == "recording",
                _ => true,
            };
            status
                && (query.is_empty()
                    || game.id.contains(&query)
                    || game
                        .map_name
                        .as_ref()
                        .is_some_and(|name| name.to_lowercase().contains(&query))
                    || game.players.iter().any(|player| {
                        player
                            .steam_name
                            .as_ref()
                            .is_some_and(|name| name.to_lowercase().contains(&query))
                            || player
                                .steam_id
                                .as_deref()
                                .is_some_and(|id| matching_ids.contains(id))
                    }))
        })
        .cloned()
        .collect()
}

fn game_row(game: &GameRecord, recorder_id: &str) -> HistoryGameUiRow {
    HistoryGameUiRow {
        id: game.id.clone().into(),
        date: date_text(&game.started_at).into(),
        map: game.map_name.as_deref().unwrap_or("Unavailable").into(),
        duration: format!("{} ticks", game.last_tick).into(),
        status: game.status.clone().into(),
        players: game
            .players
            .iter()
            .filter(|p| p.kind != "empty")
            .map(|p| {
                p.steam_name
                    .clone()
                    .unwrap_or_else(|| format!("P{}", p.slot))
            })
            .collect::<Vec<_>>()
            .join(", ")
            .into(),
        source: if game.recorder_id == recorder_id {
            "Local"
        } else {
            "Imported"
        }
        .into(),
    }
}

fn date_text(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| value.into())
}

fn player_games<'a>(state: &'a State, id: &str) -> Vec<&'a GameRecord> {
    state
        .games
        .iter()
        .filter(|game| {
            game.players
                .iter()
                .any(|player| player.steam_id.as_deref() == Some(id))
        })
        .collect()
}

fn render(app: &AppWindow, state: &State) {
    let recorder_id = state
        .recorder
        .as_ref()
        .map(Recorder::recorder_id)
        .unwrap_or("");
    let games = filtered_games(app, state);
    app.set_game_history_rows(
        Rc::new(VecModel::from(
            games
                .iter()
                .map(|game| game_row(game, recorder_id))
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
    let query = app.get_history_search();
    let players: Vec<_> = state
        .journal
        .players
        .iter()
        .filter(|player| history::search_player(player, query.as_str()))
        .map(|player| JournalPlayerUiRow {
            steam_id: player.steam_id.clone().into(),
            name: if player.latest_name.is_empty() {
                "Unnamed".into()
            } else {
                player.latest_name.clone().into()
            },
            color_index: player.color.unwrap_or(5),
            aliases: player
                .names
                .iter()
                .map(|name| name.name.clone())
                .collect::<Vec<_>>()
                .join(" · ")
                .into(),
            games: player.games.len().min(i32::MAX as usize) as i32,
            last_seen: date_text(&player.last_seen).into(),
            profile_url: format!("https://steamcommunity.com/profiles/{}/", player.steam_id).into(),
        })
        .collect();
    let player_count = players.len();
    app.set_player_journal_rows(Rc::new(VecModel::from(players)).into());
    app.set_history_list_status(format!("{} game records · {player_count} Steam accounts · Search includes previous nicknames. {}",games.len(),state.catalog_warning.as_deref().unwrap_or("")).into());
    let mut current_summary = Vec::new();
    if let Some(active) = state.recorder.as_ref().and_then(Recorder::active) {
        for participant in active
            .players
            .iter()
            .filter(|player| !player.is_self && player.steam_id.is_some())
            .take(4)
        {
            let id = participant.steam_id.as_deref().unwrap_or("");
            let prior: Vec<_> = state
                .games
                .iter()
                .filter(|game| {
                    game.id != active.id
                        && game
                            .players
                            .iter()
                            .any(|player| player.steam_id.as_deref() == Some(id))
                })
                .collect();
            let names: BTreeSet<_> = prior
                .iter()
                .flat_map(|game| game.players.iter())
                .filter(|player| player.steam_id.as_deref() == Some(id))
                .filter_map(|player| player.steam_name.clone())
                .collect();
            let name = participant
                .steam_name
                .as_deref()
                .unwrap_or("Unnamed player");
            current_summary.push(if prior.is_empty() {
                format!("{name}: first recorded encounter")
            } else {
                format!(
                    "{name}: {} prior records · names: {}",
                    prior.len(),
                    names.into_iter().collect::<Vec<_>>().join(", ")
                )
            });
        }
    }
    app.set_history_current_summary(current_summary.join("\n").into());
    if app.get_history_view() == 3 {
        if let Some(game) = state
            .games
            .iter()
            .find(|game| Some(&game.id) == state.selected_game.as_ref())
        {
            render_game(app, game);
        }
    }
    if app.get_history_view() == 4 {
        if let Some(player) = state
            .journal
            .players
            .iter()
            .find(|player| Some(&player.steam_id) == state.selected_player.as_ref())
        {
            render_player(app, state, player, recorder_id);
        }
    }
}

fn field(group: &str, label: &str, value: impl ToString) -> HistoryFieldUiRow {
    HistoryFieldUiRow {
        group: group.into(),
        label: label.into(),
        value: value.to_string().into(),
        steam_id: Default::default(),
    }
}

fn render_game(app: &AppWindow, game: &GameRecord) {
    app.set_history_detail_summary(
        format!(
            "{} · {} game ticks · recorded {}m {}s · {}",
            game.status,
            game.last_tick,
            game.observed_seconds / 60,
            game.observed_seconds % 60,
            if game.partial {
                "Partial coverage"
            } else {
                "From match start"
            }
        )
        .into(),
    );
    app.set_history_detail_title(
        format!(
            "{} · {}",
            game.map_name.as_deref().unwrap_or("Multiplayer match"),
            date_text(&game.started_at)
        )
        .into(),
    );
    let mut fields = vec![
        field("Recording", "Game ID", &game.id),
        field("Recording", "Status", &game.status),
        field("Recording", "Started", date_text(&game.started_at)),
        field("Recording", "Last statistics", date_text(&game.updated_at)),
        field(
            "Recording",
            "Finalized",
            game.ended_at
                .as_deref()
                .map(date_text)
                .unwrap_or_else(|| "Still recording".into()),
        ),
        field(
            "Recording",
            "Finish reason",
            game.finish_reason.as_deref().unwrap_or("—"),
        ),
        field("Duration", "Game ticks", game.last_tick),
        field("Duration", "First observed tick", game.first_tick),
        field(
            "Duration",
            "Observed wall time",
            format!(
                "{}m {}s",
                game.observed_seconds / 60,
                game.observed_seconds % 60
            ),
        ),
        field(
            "Duration",
            "Coverage",
            if game.partial {
                "Partial recording"
            } else {
                "From match start"
            },
        ),
        field(
            "Setup",
            "Map",
            game.map_name.as_deref().unwrap_or("Unavailable"),
        ),
        field(
            "Setup",
            "Map size",
            game.map_size
                .map(|size| size.to_string())
                .unwrap_or_else(|| "Unavailable".into()),
        ),
    ];
    for (label, value) in &game.setup {
        fields.push(field("Setup", label, display_value(value)));
    }
    app.set_history_detail_fields(Rc::new(VecModel::from(fields)).into());
    app.set_history_roster_rows(
        Rc::new(VecModel::from(
            game.players
                .iter()
                .map(|player| {
                    let stat = |name: &str| {
                        if player.kind == "empty" {
                            return slint::SharedString::from("—");
                        }
                        player
                            .statistics
                            .get(name)
                            .map(display_value)
                            .unwrap_or_else(|| "—".into())
                            .into()
                    };
                    HistoryRosterUiRow {
                        slot: player.slot as i32,
                        name: player
                            .steam_name
                            .clone()
                            .unwrap_or_else(|| match player.kind.as_str() {
                                "empty" => "Empty slot".into(),
                                "ai" => format!("AI P{}", player.slot),
                                _ => format!("P{}", player.slot),
                            })
                            .into(),
                        steam_id: player.steam_id.as_deref().unwrap_or("").into(),
                        color_index: player.color.unwrap_or(5),
                        team: player
                            .team
                            .map(|team| team.to_string())
                            .unwrap_or_else(|| "—".into())
                            .into(),
                        gold: stat("gold"),
                        population: stat("population"),
                        army: stat("army"),
                        kills: stat("army_killed"),
                        losses: stat("army_lost"),
                        acquired_gold: stat("stat_gold_earned"),
                        food_produced: stat("stat_food_produced"),
                        weapons_produced: stat("stat_weapons_produced"),
                    }
                })
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
}

fn render_player(app: &AppWindow, state: &State, player: &JournalPlayer, recorder_id: &str) {
    app.set_history_detail_summary(format!("{} game records · {} locally recorded · {} observed nicknames · confirmed results: {} wins / {} losses",player.games.len(),player.local_games,player.names.len(),player.wins,player.losses).into());
    app.set_history_detail_title(if player.latest_name.is_empty() {
        "Unnamed player".into()
    } else {
        player.latest_name.clone().into()
    });
    app.set_history_detail_steam_id(player.steam_id.clone().into());
    let mut account = field("Account", "SteamID64", &player.steam_id);
    account.steam_id = player.steam_id.clone().into();
    let fields = vec![
        account,
        field("Account", "Latest recorded name", &player.latest_name),
        field("Account", "First recorded", date_text(&player.first_seen)),
        field("Account", "Last recorded", date_text(&player.last_seen)),
        field("Statistics", "Game records", player.games.len()),
        field("Statistics", "Locally recorded", player.local_games),
        field("Statistics", "Confirmed wins", player.wins),
        field("Statistics", "Confirmed losses", player.losses),
        field("Statistics", "Army kills", player.kills),
        field("Statistics", "Army lost", player.units_lost),
        field("Statistics", "Acquired gold", player.acquired_gold),
    ];
    app.set_history_detail_fields(Rc::new(VecModel::from(fields)).into());
    app.set_history_player_names(
        Rc::new(VecModel::from(
            player
                .names
                .iter()
                .map(|name| {
                    format!(
                        "{}    ·    {} → {}",
                        name.name,
                        date_text(&name.first_seen),
                        date_text(&name.last_seen)
                    )
                    .into()
                })
                .collect::<Vec<slint::SharedString>>(),
        ))
        .into(),
    );
    app.set_history_player_games(
        Rc::new(VecModel::from(
            player_games(state, &player.steam_id)
                .into_iter()
                .map(|game| game_row(game, recorder_id))
                .collect::<Vec<_>>(),
        ))
        .into(),
    );
}

fn display_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => "Unavailable".into(),
        _ => value.to_string(),
    }
}

#[cfg(test)]
pub(crate) fn render_test_fixtures(app: &AppWindow, mut capture: impl FnMut(&str)) {
    use crate::appearance_tests::click;
    use slint::Model;
    let profile_clicks = Rc::new(RefCell::new(Vec::new()));
    app.on_history_open_player_profile({
        let clicks = profile_clicks.clone();
        move |id| clicks.borrow_mut().push(id.to_string())
    });
    let journal_clicks = Rc::new(RefCell::new(Vec::new()));
    app.on_history_open_player({
        let clicks = journal_clicks.clone();
        move |id| clicks.borrow_mut().push(id.to_string())
    });
    let mut first = history::tests::record("11111111-1111-4111-8111-111111111111", "Old nickname");
    first.status = "finished".into();
    first.ended_at = Some(first.updated_at.clone());
    first.last_tick = 72_500;
    first.observed_seconds = 1450;
    first
        .setup
        .insert("Peacetime".into(), serde_json::json!(10));
    first
        .setup
        .insert("Starting goods".into(), serde_json::json!(2000));
    for (index, player) in first.players.iter_mut().enumerate().take(3) {
        player.kind = if index == 2 { "ai" } else { "human" }.into();
        player.color = Some(index as i32 + 4);
        player.team = Some(index as i32 + 1);
        player.statistics = serde_json::json!({"gold":12000-index*1000,"population":250,"army":180,"army_killed":70,"army_lost":30,"stat_gold_earned":18000,"stat_food_produced":350,"stat_weapons_produced":125});
        if index == 1 {
            player.steam_name = Some("Рустам".into());
            player.steam_id = Some("76561198027391270".into());
        }
        if index == 2 {
            player.steam_name = Some("The Rat (AI)".into());
        }
    }
    let mut second = first.clone();
    second.id = "33333333-3333-4333-8333-333333333333".into();
    second.started_at = "2026-11-02T11:30:00.000Z".into();
    second.updated_at = "2026-11-02T11:55:00.000Z".into();
    second.ended_at = Some(second.updated_at.clone());
    second.players[0].steam_name = Some("New nickname".into());
    second.map_name = Some("Desert crossing".into());
    let mut state = State::new();
    state.games = vec![second.clone(), first];
    state.journal = history::build_journal(&state.games, &second.recorder_id);
    state.selected_game = Some(second.id.clone());
    state.selected_player = second.players[0].steam_id.clone();
    app.set_admin_mode(true);
    app.set_settings_tab(3);
    app.set_history_status(
        "Sample records · one JSON per game · five-second saves while playing".into(),
    );
    for (view, suffix) in [
        (1, "-games"),
        (2, "-players"),
        (3, "-game-details"),
        (4, "-player-details"),
    ] {
        app.set_history_search("".into());
        app.set_history_view(view);
        render(app, &state);
        capture(suffix);
        if view == 2 {
            click(app, 1080., 164.);
            assert_eq!(&*profile_clicks.borrow(), &["76561198027391269"]);
            assert!(
                journal_clicks.borrow().is_empty(),
                "The row must not intercept an ID click"
            );
            click(app, 640., 155.);
            assert_eq!(&*journal_clicks.borrow(), &["76561198027391269"]);
        } else if view == 3 {
            click(app, 900., 210.);
            click(app, 900., 315.); // Empty-slot dash is not a link.
            assert_eq!(profile_clicks.borrow().len(), 2);
            click(app, 680., 210.);
            assert_eq!(journal_clicks.borrow().len(), 2);
        } else if view == 4 {
            click(app, 750., 80.);
            assert_eq!(profile_clicks.borrow().len(), 3);
            click(app, 1000., 470.);
            capture("-player-account-field");
            click(app, 830., 535.);
            assert_eq!(
                profile_clicks.borrow().len(),
                4,
                "Expanded account SteamID must open its profile"
            );
            click(app, 1000., 470.);
        }
    }
    app.set_history_view(2);
    app.set_history_search("OLD NICKNAME".into());
    render(app, &state);
    assert_eq!(app.get_player_journal_rows().row_count(), 1);
    assert_eq!(
        app.get_player_journal_rows().row_data(0).unwrap().name,
        "New nickname"
    );
    assert_eq!(
        app.get_game_history_rows().row_count(),
        2,
        "An old name must find both games by the same account"
    );
    capture("-old-name-search");
    let runtime = install(
        app,
        Rc::new(RefCell::new(LiveStatsSource::new())),
        Rc::new(RefCell::new(AppConfig::default())),
    );
    app.invoke_history_rebuild_players();
    app.invoke_history_import();
    app.invoke_history_open_folder();
    assert_eq!(app.get_history_view(), 0);
    assert_eq!(app.get_player_journal_rows().row_count(), 0);
    assert_eq!(app.get_history_status(), "Admin key required.");
    runtime.finish().unwrap();
}
