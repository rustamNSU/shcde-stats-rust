#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod engine;
mod obfuscation;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use config::{AppConfig, ColumnVisibility, SavedWindowPosition, ShooterTypeVisibility};
use engine::service::LiveStatsSource;
use engine::snapshot::{overlay_rows, GameSnapshot};
use obfuscation::{admin_password_config_value, admin_password_is_valid};
use slint::{
    CloseRequestResponse, ComponentHandle, Model, PhysicalPosition, PhysicalSize, Timer, TimerMode,
    VecModel,
};
use ui::bridge::{to_player_ui_rows, ShooterUiOptions};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let app = AppWindow::new()?;
    let overlay = OverlayWindow::new()?;
    let timer_window = TimerWindow::new()?;
    let admin_window = AdminWindow::new()?;
    let config = AppConfig::load_or_create();
    apply_config(&app, &config);
    apply_window_positions(&app, &overlay, &timer_window, &config);
    let config_state = Rc::new(RefCell::new(config));

    app.set_process_status("Read-only. Looking for SHC:DE process.".into());

    let source = Rc::new(RefCell::new(LiveStatsSource::new()));
    source
        .borrow_mut()
        .set_heavy_refresh_interval_ms(app.get_heavy_refresh_interval_ms() as u64);
    let initial_snapshot = source.borrow_mut().snapshot();
    let initial_rows = to_player_ui_rows(
        &initial_snapshot,
        &player_names(&app),
        &visible_players(&app),
        &player_colors(&app),
        &shooter_options(&app),
    );
    app.set_process_status(source.borrow().status_text().into());
    let player_rows = Rc::new(VecModel::from(initial_rows));
    app.set_player_rows(player_rows.clone().into());
    set_overlay_rows(&overlay, &visible_overlay_rows(&player_rows));
    sync_overlay_settings(&app, &overlay);
    sync_timer_settings(&app, &timer_window, &initial_snapshot);
    let manual_admin_ui_player_id = Rc::new(RefCell::new(None::<u32>));
    sync_admin_memory_player(
        &app,
        &manual_admin_ui_player_id,
        initial_snapshot.game.ui_player_id,
    );
    sync_admin_window(&app, &admin_window, &initial_snapshot);

    app.window().on_close_requested({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let admin_window = admin_window.as_weak();
        let config_state = config_state.clone();
        move || {
            if let (Some(app), Some(overlay), Some(timer_window), Some(admin_window)) = (
                app.upgrade(),
                overlay.upgrade(),
                timer_window.upgrade(),
                admin_window.upgrade(),
            ) {
                save_current_config(&config_state, &app, &overlay, &timer_window);
                overlay.hide().ok();
                timer_window.hide().ok();
                admin_window.hide().ok();
            }

            CloseRequestResponse::HideWindow
        }
    });

    app.on_show_overlay({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let config_state = config_state.clone();
        move || {
            let (Some(app), Some(overlay)) = (app.upgrade(), overlay.upgrade()) else {
                return;
            };

            sync_overlay_settings(&app, &overlay);
            if let Err(err) = overlay.show() {
                app.set_process_status(format!("Could not show overlay: {err}").into());
            } else {
                apply_window_position(&overlay.window(), &config_state.borrow().overlay_window);
                overlay.window().request_redraw();
            }
        }
    });

    app.on_hide_overlay({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let config_state = config_state.clone();
        move || {
            if let (Some(app), Some(overlay), Some(timer_window)) =
                (app.upgrade(), overlay.upgrade(), timer_window.upgrade())
            {
                save_current_config(&config_state, &app, &overlay, &timer_window);
                overlay.hide().ok();
            }
        }
    });

    app.on_show_timer({
        let app = app.as_weak();
        let timer_window = timer_window.as_weak();
        let config_state = config_state.clone();
        move || {
            let (Some(app), Some(timer_window)) = (app.upgrade(), timer_window.upgrade()) else {
                return;
            };

            sync_timer_panel_settings(&app, &timer_window);
            if let Err(err) = timer_window.show() {
                app.set_process_status(format!("Could not show timer: {err}").into());
            } else {
                apply_window_position(&timer_window.window(), &config_state.borrow().timer_window);
                timer_window.window().request_redraw();
            }
        }
    });

    app.on_hide_timer({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let config_state = config_state.clone();
        move || {
            if let (Some(app), Some(overlay), Some(timer_window)) =
                (app.upgrade(), overlay.upgrade(), timer_window.upgrade())
            {
                save_current_config(&config_state, &app, &overlay, &timer_window);
                timer_window.hide().ok();
            }
        }
    });

    app.on_show_admin({
        let app = app.as_weak();
        let admin_window = admin_window.as_weak();
        move || {
            let (Some(app), Some(admin_window)) = (app.upgrade(), admin_window.upgrade()) else {
                return;
            };

            if !app.get_admin_mode() {
                app.set_process_status("Admin key required. Read-only mode.".into());
                return;
            }

            sync_admin_panel_settings(&app, &admin_window);
            if let Err(err) = admin_window.show() {
                app.set_process_status(format!("Could not show admin window: {err}").into());
            } else {
                admin_window.window().request_redraw();
            }
        }
    });

    app.on_hide_admin({
        let admin_window = admin_window.as_weak();
        move || {
            if let Some(admin_window) = admin_window.upgrade() {
                admin_window.hide().ok();
            }
        }
    });

    app.on_cycle_player_color({
        let app = app.as_weak();
        move |player_id| {
            let Some(app) = app.upgrade() else {
                return;
            };

            cycle_player_color(&app, player_id);
        }
    });

    app.on_submit_admin_key({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let config_state = config_state.clone();
        move |key| {
            let (Some(app), Some(overlay), Some(timer_window)) =
                (app.upgrade(), overlay.upgrade(), timer_window.upgrade())
            else {
                return;
            };

            if admin_password_is_valid(key.as_str()) {
                app.set_admin_mode(true);
                app.set_process_status("Admin mode unlocked.".into());
                config_state.borrow_mut().admin_password =
                    admin_password_config_value(key.as_str());
                save_current_config(&config_state, &app, &overlay, &timer_window);
            } else {
                app.set_process_status("Invalid admin key. Read-only mode.".into());
            }
        }
    });

    app.on_request_admin_ui_player({
        let app = app.as_weak();
        let source = source.clone();
        let manual_admin_ui_player_id = manual_admin_ui_player_id.clone();
        move |player_id| {
            let Some(app) = app.upgrade() else {
                return;
            };

            write_admin_ui_player(&app, &source, &manual_admin_ui_player_id, player_id);
        }
    });

    overlay.on_hide_overlay({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let config_state = config_state.clone();
        move || {
            if let (Some(app), Some(overlay), Some(timer_window)) =
                (app.upgrade(), overlay.upgrade(), timer_window.upgrade())
            {
                save_current_config(&config_state, &app, &overlay, &timer_window);
                overlay.hide().ok();
            }
        }
    });

    overlay.on_request_overlay_frame({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        move |enabled| {
            let (Some(app), Some(overlay)) = (app.upgrade(), overlay.upgrade()) else {
                return;
            };

            app.set_overlay_frame(enabled);
            sync_overlay_settings(&app, &overlay);
        }
    });

    timer_window.on_hide_timer({
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let config_state = config_state.clone();
        move || {
            if let (Some(app), Some(overlay), Some(timer_window)) =
                (app.upgrade(), overlay.upgrade(), timer_window.upgrade())
            {
                save_current_config(&config_state, &app, &overlay, &timer_window);
                timer_window.hide().ok();
            }
        }
    });

    timer_window.on_request_timer_frame({
        let app = app.as_weak();
        let timer_window = timer_window.as_weak();
        move |enabled| {
            let (Some(app), Some(timer_window)) = (app.upgrade(), timer_window.upgrade()) else {
                return;
            };

            app.set_timer_frame(enabled);
            sync_timer_panel_settings(&app, &timer_window);
        }
    });

    timer_window.on_request_timer_player({
        let app = app.as_weak();
        move |delta| {
            let Some(app) = app.upgrade() else {
                return;
            };

            let next_player_id = (app.get_timer_player_id() + delta).clamp(1, 8);
            app.set_timer_player_id(next_player_id);
        }
    });

    admin_window.on_hide_admin({
        let admin_window = admin_window.as_weak();
        move || {
            if let Some(admin_window) = admin_window.upgrade() {
                admin_window.hide().ok();
            }
        }
    });

    admin_window.on_request_admin_frame({
        let app = app.as_weak();
        let admin_window = admin_window.as_weak();
        move |enabled| {
            let (Some(app), Some(admin_window)) = (app.upgrade(), admin_window.upgrade()) else {
                return;
            };

            app.set_admin_frame(enabled);
            sync_admin_panel_settings(&app, &admin_window);
        }
    });

    admin_window.on_request_admin_player({
        let app = app.as_weak();
        let source = source.clone();
        let manual_admin_ui_player_id = manual_admin_ui_player_id.clone();
        move |delta| {
            let Some(app) = app.upgrade() else {
                return;
            };

            let next_player_id = (app.get_admin_ui_player_id() + delta).clamp(1, 8);
            write_admin_ui_player(&app, &source, &manual_admin_ui_player_id, next_player_id);
        }
    });

    let stats_timer = Timer::default();
    stats_timer.start(TimerMode::Repeated, Duration::from_millis(100), {
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let admin_window = admin_window.as_weak();
        let source = source.clone();
        let player_rows = player_rows.clone();
        let manual_admin_ui_player_id = manual_admin_ui_player_id.clone();
        move || {
            let (Some(app), Some(overlay), Some(timer_window), Some(admin_window)) = (
                app.upgrade(),
                overlay.upgrade(),
                timer_window.upgrade(),
                admin_window.upgrade(),
            ) else {
                return;
            };

            let mut source = source.borrow_mut();
            source.set_heavy_refresh_interval_ms(app.get_heavy_refresh_interval_ms() as u64);
            let snapshot = source.snapshot();
            app.set_process_status(source.status_text().into());
            sync_admin_memory_player(&app, &manual_admin_ui_player_id, snapshot.game.ui_player_id);
            let rows = to_player_ui_rows(
                &snapshot,
                &player_names(&app),
                &visible_players(&app),
                &player_colors(&app),
                &shooter_options(&app),
            );

            for (index, row) in rows.into_iter().enumerate() {
                if index < player_rows.row_count() {
                    player_rows.set_row_data(index, row);
                } else {
                    player_rows.push(row);
                }
            }

            set_overlay_rows(&overlay, &visible_overlay_rows(&player_rows));
            sync_overlay_settings(&app, &overlay);
            sync_timer_settings(&app, &timer_window, &snapshot);
            sync_admin_window(&app, &admin_window, &snapshot);
        }
    });

    let config_timer = Timer::default();
    config_timer.start(TimerMode::Repeated, Duration::from_millis(1000), {
        let app = app.as_weak();
        let overlay = overlay.as_weak();
        let timer_window = timer_window.as_weak();
        let config_state = config_state.clone();
        move || {
            let (Some(app), Some(overlay), Some(timer_window)) =
                (app.upgrade(), overlay.upgrade(), timer_window.upgrade())
            else {
                return;
            };

            save_current_config(&config_state, &app, &overlay, &timer_window);
        }
    });

    let result = app.run();
    save_current_config(&config_state, &app, &overlay, &timer_window);
    result
}

fn apply_config(app: &AppWindow, config: &AppConfig) {
    app.set_tournament_title(config.title.clone().into());
    app.set_match_label(config.description.clone().into());
    app.set_pin_on_top(config.pin_on_top);
    app.set_admin_mode(config.admin_mode_enabled());
    app.set_admin_key_input("".into());
    app.set_settings_tab(config.settings_tab);

    if let Some(player) = config.players.first() {
        app.set_player1_name(player.name.clone().into());
        app.set_show_player1(player.visible);
        app.set_player1_color(player.color);
    }
    if let Some(player) = config.players.get(1) {
        app.set_player2_name(player.name.clone().into());
        app.set_show_player2(player.visible);
        app.set_player2_color(player.color);
    }
    if let Some(player) = config.players.get(2) {
        app.set_player3_name(player.name.clone().into());
        app.set_show_player3(player.visible);
        app.set_player3_color(player.color);
    }
    if let Some(player) = config.players.get(3) {
        app.set_player4_name(player.name.clone().into());
        app.set_show_player4(player.visible);
        app.set_player4_color(player.color);
    }
    if let Some(player) = config.players.get(4) {
        app.set_player5_name(player.name.clone().into());
        app.set_show_player5(player.visible);
        app.set_player5_color(player.color);
    }
    if let Some(player) = config.players.get(5) {
        app.set_player6_name(player.name.clone().into());
        app.set_show_player6(player.visible);
        app.set_player6_color(player.color);
    }
    if let Some(player) = config.players.get(6) {
        app.set_player7_name(player.name.clone().into());
        app.set_show_player7(player.visible);
        app.set_player7_color(player.color);
    }
    if let Some(player) = config.players.get(7) {
        app.set_player8_name(player.name.clone().into());
        app.set_show_player8(player.visible);
        app.set_player8_color(player.color);
    }

    app.set_panel_opacity(config.overlay.panel_opacity);
    app.set_overlay_font_size(config.overlay.font_size);
    app.set_heavy_refresh_interval_ms(config.overlay.heavy_read_ms);
    app.set_overlay_frame(config.overlay.movable_frame);

    app.set_timer_panel_opacity(config.timer.panel_opacity);
    app.set_timer_font_size(config.timer.font_size);
    app.set_game_speed(quantized_game_speed(config.timer.game_speed));
    app.set_timer_frame(config.timer.movable_frame);

    apply_column_config(app, &config.columns);
    apply_shooter_config(app, &config.shooter_types);
}

fn apply_column_config(app: &AppWindow, columns: &ColumnVisibility) {
    app.set_show_gold(columns.gold);
    app.set_show_population(columns.population);
    app.set_show_army(columns.army);
    app.set_show_food_effect(columns.food_effect);
    app.set_show_tax_effect(columns.tax_effect);
    app.set_show_ha(columns.ha);
    app.set_show_cl(columns.cl);
    app.set_show_shooters(columns.shooters);
    app.set_show_acq_gold(columns.acquired_gold);
    app.set_show_prod_weapons(columns.produced_weapons);
    app.set_show_prod_food(columns.produced_food);
    app.set_show_prod_stone(columns.produced_stone);
    app.set_show_prod_iron(columns.produced_iron);
    app.set_show_prod_wood(columns.produced_wood);
    app.set_show_total_effect(columns.total_effect);
    app.set_show_knights(columns.knights);
    app.set_show_ff(columns.fear_factor);
    app.set_show_army_killed(columns.army_killed);
    app.set_show_army_lost(columns.army_lost);
}

fn apply_shooter_config(app: &AppWindow, shooter_types: &ShooterTypeVisibility) {
    app.set_shooter_count_skirmishers(shooter_types.skirmishers);
    app.set_shooter_count_archers(shooter_types.archers);
    app.set_shooter_count_xbows(shooter_types.xbows);
    app.set_shooter_count_arab_bows(shooter_types.arab_bows);
    app.set_shooter_count_slingers(shooter_types.slingers);
}

fn apply_window_positions(
    app: &AppWindow,
    _overlay: &OverlayWindow,
    _timer_window: &TimerWindow,
    config: &AppConfig,
) {
    apply_window_geometry(&app.window(), &config.main_window);
}

fn apply_window_geometry(window: &slint::Window, position: &SavedWindowPosition) {
    apply_window_position(window, position);

    if position.saved && position.width > 0 && position.height > 0 {
        window.set_size(PhysicalSize::new(position.width, position.height));
    }
}

fn apply_window_position(window: &slint::Window, position: &SavedWindowPosition) {
    if position.is_usable() {
        window.set_position(PhysicalPosition::new(position.x, position.y));
    }
}

fn save_current_config(
    config_state: &Rc<RefCell<AppConfig>>,
    app: &AppWindow,
    overlay: &OverlayWindow,
    timer_window: &TimerWindow,
) {
    let previous = config_state.borrow().clone();
    let mut config = collect_config(app, overlay, timer_window, &previous);
    config.normalize();

    if config.save().is_ok() {
        *config_state.borrow_mut() = config;
    }
}

fn collect_config(
    app: &AppWindow,
    overlay: &OverlayWindow,
    timer_window: &TimerWindow,
    previous: &AppConfig,
) -> AppConfig {
    AppConfig {
        title: app.get_tournament_title().to_string(),
        description: app.get_match_label().to_string(),
        players: player_config(app),
        pin_on_top: app.get_pin_on_top(),
        admin_password: if app.get_admin_mode() {
            previous
                .admin_password
                .as_deref()
                .and_then(admin_password_config_value)
        } else {
            None
        },
        settings_tab: app.get_settings_tab(),
        overlay: config::OverlaySettings {
            panel_opacity: app.get_panel_opacity(),
            font_size: app.get_overlay_font_size(),
            heavy_read_ms: app.get_heavy_refresh_interval_ms(),
            movable_frame: app.get_overlay_frame(),
        },
        timer: config::TimerSettings {
            panel_opacity: app.get_timer_panel_opacity(),
            font_size: app.get_timer_font_size(),
            game_speed: quantized_game_speed(app.get_game_speed()),
            movable_frame: app.get_timer_frame(),
        },
        columns: ColumnVisibility {
            gold: app.get_show_gold(),
            population: app.get_show_population(),
            army: app.get_show_army(),
            food_effect: app.get_show_food_effect(),
            tax_effect: app.get_show_tax_effect(),
            ha: app.get_show_ha(),
            cl: app.get_show_cl(),
            shooters: app.get_show_shooters(),
            acquired_gold: app.get_show_acq_gold(),
            produced_weapons: app.get_show_prod_weapons(),
            produced_food: app.get_show_prod_food(),
            produced_stone: app.get_show_prod_stone(),
            produced_iron: app.get_show_prod_iron(),
            produced_wood: app.get_show_prod_wood(),
            total_effect: app.get_show_total_effect(),
            knights: app.get_show_knights(),
            fear_factor: app.get_show_ff(),
            army_killed: app.get_show_army_killed(),
            army_lost: app.get_show_army_lost(),
        },
        shooter_types: ShooterTypeVisibility {
            skirmishers: app.get_shooter_count_skirmishers(),
            archers: app.get_shooter_count_archers(),
            xbows: app.get_shooter_count_xbows(),
            arab_bows: app.get_shooter_count_arab_bows(),
            slingers: app.get_shooter_count_slingers(),
        },
        main_window: current_window_position(&app.window()),
        overlay_window: current_or_previous_window_position(
            &overlay.window(),
            &previous.overlay_window,
        ),
        timer_window: current_or_previous_window_position(
            &timer_window.window(),
            &previous.timer_window,
        ),
    }
}

fn write_admin_ui_player(
    app: &AppWindow,
    source: &Rc<RefCell<LiveStatsSource>>,
    manual_admin_ui_player_id: &Rc<RefCell<Option<u32>>>,
    player_id: i32,
) {
    if !app.get_admin_mode() {
        app.set_process_status("Admin key required. Read-only mode.".into());
        return;
    }

    let player_id = player_id.clamp(1, 8) as u32;
    match source.borrow_mut().write_ui_player_id(player_id) {
        Ok(()) => {
            *manual_admin_ui_player_id.borrow_mut() = Some(player_id);
            app.set_admin_ui_player_id(player_id as i32);
            app.set_process_status(format!("Admin: UI player id set to {player_id}.").into());
        }
        Err(err) => {
            app.set_process_status(format!("Admin write failed: {err:#}").into());
        }
    }
}

fn sync_admin_memory_player(
    app: &AppWindow,
    manual_admin_ui_player_id: &Rc<RefCell<Option<u32>>>,
    live_ui_player_id: u32,
) {
    if !(1..=8).contains(&live_ui_player_id) {
        return;
    }

    if *manual_admin_ui_player_id.borrow() == Some(live_ui_player_id) {
        return;
    }

    *manual_admin_ui_player_id.borrow_mut() = None;
    app.set_admin_game_player_id(live_ui_player_id as i32);
    app.set_admin_ui_player_id(live_ui_player_id as i32);
}

fn sync_admin_window(app: &AppWindow, admin_window: &AdminWindow, snapshot: &GameSnapshot) {
    sync_admin_panel_settings(app, admin_window);

    let game_player_id = app.get_admin_game_player_id().clamp(1, 8);
    let selected_player_id = app.get_admin_ui_player_id().clamp(1, 8) as u32;
    if app.get_admin_ui_player_id() != selected_player_id as i32 {
        app.set_admin_ui_player_id(selected_player_id as i32);
    }

    let row = overlay_rows(snapshot)
        .into_iter()
        .find(|row| u32::from(row.player_id) == selected_player_id);

    admin_window.set_game_player_text(format!("P{game_player_id}").into());
    admin_window.set_selected_player_text(format!("P{selected_player_id}").into());
    admin_window.set_gold_text(
        row.as_ref()
            .map(|row| row.gold)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_population_text(
        row.as_ref()
            .map(|row| row.population)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_army_text(
        row.as_ref()
            .map(|row| row.army)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_ff_text(
        row.as_ref()
            .map(|row| row.ff)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_acquired_gold_text(
        row.as_ref()
            .map(|row| row.stat_gold_earned)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_produced_food_text(
        row.as_ref()
            .map(|row| row.stat_food_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_produced_stone_text(
        row.as_ref()
            .map(|row| row.stat_stone_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_produced_iron_text(
        row.as_ref()
            .map(|row| row.stat_iron_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_produced_wood_text(
        row.as_ref()
            .map(|row| row.stat_wood_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    admin_window.set_produced_weapons_text(
        row.as_ref()
            .map(|row| row.stat_weapons_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
}

fn sync_admin_panel_settings(app: &AppWindow, admin_window: &AdminWindow) {
    admin_window.set_panel_opacity(app.get_timer_panel_opacity());
    admin_window.set_admin_font_size(app.get_timer_font_size());
    admin_window.set_admin_frame(app.get_admin_frame());
    admin_window.set_pin_on_top(app.get_pin_on_top());
}

fn player_config(app: &AppWindow) -> Vec<config::PlayerSlotConfig> {
    vec![
        config::PlayerSlotConfig {
            name: app.get_player1_name().to_string(),
            visible: app.get_show_player1(),
            color: app.get_player1_color(),
        },
        config::PlayerSlotConfig {
            name: app.get_player2_name().to_string(),
            visible: app.get_show_player2(),
            color: app.get_player2_color(),
        },
        config::PlayerSlotConfig {
            name: app.get_player3_name().to_string(),
            visible: app.get_show_player3(),
            color: app.get_player3_color(),
        },
        config::PlayerSlotConfig {
            name: app.get_player4_name().to_string(),
            visible: app.get_show_player4(),
            color: app.get_player4_color(),
        },
        config::PlayerSlotConfig {
            name: app.get_player5_name().to_string(),
            visible: app.get_show_player5(),
            color: app.get_player5_color(),
        },
        config::PlayerSlotConfig {
            name: app.get_player6_name().to_string(),
            visible: app.get_show_player6(),
            color: app.get_player6_color(),
        },
        config::PlayerSlotConfig {
            name: app.get_player7_name().to_string(),
            visible: app.get_show_player7(),
            color: app.get_player7_color(),
        },
        config::PlayerSlotConfig {
            name: app.get_player8_name().to_string(),
            visible: app.get_show_player8(),
            color: app.get_player8_color(),
        },
    ]
}

fn current_or_previous_window_position(
    window: &slint::Window,
    previous: &SavedWindowPosition,
) -> SavedWindowPosition {
    if window.is_visible() {
        let current = current_window_position(window);
        if current.is_usable() {
            current
        } else if previous.is_usable() {
            previous.clone()
        } else {
            SavedWindowPosition::default()
        }
    } else if previous.is_usable() {
        previous.clone()
    } else {
        SavedWindowPosition::default()
    }
}

fn current_window_position(window: &slint::Window) -> SavedWindowPosition {
    let position = window.position();
    let size = window.size();
    let mut saved = SavedWindowPosition {
        saved: true,
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    saved.normalize();
    saved
}

fn visible_overlay_rows(player_rows: &VecModel<PlayerUiRow>) -> Vec<PlayerUiRow> {
    (0..player_rows.row_count())
        .filter_map(|index| player_rows.row_data(index))
        .filter(|row| row.visible)
        .collect()
}

fn set_overlay_rows(overlay: &OverlayWindow, rows: &[PlayerUiRow]) {
    overlay.set_player_rows(Rc::new(VecModel::from(rows.to_vec())).into());
}

fn sync_overlay_settings(app: &AppWindow, overlay: &OverlayWindow) {
    let visible = visible_players(app);
    let (width, height) = overlay_size(
        visible.iter().filter(|visible| **visible).count(),
        app.get_show_gold(),
        app.get_show_population(),
        app.get_show_army(),
        app.get_show_food_effect(),
        app.get_show_tax_effect(),
        app.get_show_ha(),
        app.get_show_cl(),
        app.get_show_shooters(),
        app.get_show_acq_gold(),
        app.get_show_prod_food(),
        app.get_show_prod_weapons(),
        app.get_show_prod_stone(),
        app.get_show_prod_iron(),
        app.get_show_prod_wood(),
        app.get_show_total_effect(),
        app.get_show_knights(),
        app.get_show_ff(),
        app.get_show_army_killed(),
        app.get_show_army_lost(),
        app.get_overlay_font_size(),
    );

    overlay.set_tournament_title(app.get_tournament_title());
    overlay.set_match_label(app.get_match_label());
    overlay.set_panel_opacity(app.get_panel_opacity());
    overlay.set_overlay_font_size(app.get_overlay_font_size());
    overlay.set_overlay_frame(app.get_overlay_frame());
    overlay.set_pin_on_top(app.get_pin_on_top());
    overlay.set_show_gold(app.get_show_gold());
    overlay.set_show_population(app.get_show_population());
    overlay.set_show_army(app.get_show_army());
    overlay.set_show_food_effect(app.get_show_food_effect());
    overlay.set_show_tax_effect(app.get_show_tax_effect());
    overlay.set_show_ha(app.get_show_ha());
    overlay.set_show_cl(app.get_show_cl());
    overlay.set_show_shooters(app.get_show_shooters());
    overlay.set_show_acq_gold(app.get_show_acq_gold());
    overlay.set_show_prod_food(app.get_show_prod_food());
    overlay.set_show_prod_weapons(app.get_show_prod_weapons());
    overlay.set_show_prod_stone(app.get_show_prod_stone());
    overlay.set_show_prod_iron(app.get_show_prod_iron());
    overlay.set_show_prod_wood(app.get_show_prod_wood());
    overlay.set_show_total_effect(app.get_show_total_effect());
    overlay.set_show_knights(app.get_show_knights());
    overlay.set_show_ff(app.get_show_ff());
    overlay.set_show_army_killed(app.get_show_army_killed());
    overlay.set_show_army_lost(app.get_show_army_lost());
    overlay.set_overlay_width(width);
    overlay.set_overlay_height(height);
}

fn sync_timer_settings(app: &AppWindow, timer_window: &TimerWindow, snapshot: &GameSnapshot) {
    sync_timer_panel_settings(app, timer_window);

    let game = &snapshot.game;
    let game_speed = quantized_game_speed(app.get_game_speed());
    timer_window.set_ticks_text(game.ticks.to_string().into());
    timer_window.set_day_text((game.month_part + 1).to_string().into());
    timer_window.set_month_text(month_text(game.month).into());
    timer_window.set_year_text(game.year.to_string().into());
    timer_window.set_elapsed_text(elapsed_time_text(game.ticks, game_speed).into());
    timer_window.set_game_speed_text(format!("{game_speed:.0}").into());

    let player_id = app.get_timer_player_id().clamp(1, 8) as u32;
    if app.get_timer_player_id() != player_id as i32 {
        app.set_timer_player_id(player_id as i32);
    }

    let stats = snapshot
        .post_game_statistics
        .players
        .iter()
        .find(|stats| stats.player_id == player_id);

    timer_window.set_timer_player_text(format!("P{player_id}").into());
    timer_window.set_acquired_gold_text(
        stats
            .map(|stats| stats.gold_earning)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    timer_window.set_produced_food_text(
        stats
            .map(|stats| stats.food_score)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    timer_window.set_produced_stone_text(
        stats
            .map(|stats| stats.total_produced_stone)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    timer_window.set_produced_iron_text(
        stats
            .map(|stats| stats.iron_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    timer_window.set_produced_wood_text(
        stats
            .map(|stats| stats.wood_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
    timer_window.set_produced_weapons_text(
        stats
            .map(|stats| stats.weapons_produced)
            .unwrap_or_default()
            .to_string()
            .into(),
    );
}

fn sync_timer_panel_settings(app: &AppWindow, timer_window: &TimerWindow) {
    let game_speed = quantized_game_speed(app.get_game_speed());
    if (app.get_game_speed() - game_speed).abs() > f32::EPSILON {
        app.set_game_speed(game_speed);
    }

    timer_window.set_panel_opacity(app.get_timer_panel_opacity());
    timer_window.set_timer_font_size(app.get_timer_font_size());
    timer_window.set_timer_frame(app.get_timer_frame());
    timer_window.set_pin_on_top(app.get_pin_on_top());
}

fn quantized_game_speed(value: f32) -> f32 {
    ((value / 5.0).round() * 5.0).clamp(40.0, 90.0)
}

fn elapsed_time_text(ticks: u32, game_speed: f32) -> String {
    let total_seconds = (f64::from(ticks) / f64::from(game_speed.max(1.0))).floor() as u64;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    format!("{minutes:02}:{seconds:02}")
}

fn month_text(month: u32) -> &'static str {
    match month {
        0 => "Jan",
        1 => "Feb",
        2 => "Mar",
        3 => "Apr",
        4 => "May",
        5 => "Jun",
        6 => "Jul",
        7 => "Aug",
        8 => "Sep",
        9 => "Oct",
        10 => "Nov",
        11 => "Dec",
        _ => "---",
    }
}

fn overlay_size(
    visible_player_count: usize,
    show_gold: bool,
    show_population: bool,
    show_army: bool,
    show_food_effect: bool,
    show_tax_effect: bool,
    show_ha: bool,
    show_cl: bool,
    show_shooters: bool,
    show_acq_gold: bool,
    show_prod_food: bool,
    show_prod_weapons: bool,
    show_prod_stone: bool,
    show_prod_iron: bool,
    show_prod_wood: bool,
    show_total_effect: bool,
    show_knights: bool,
    show_ff: bool,
    show_army_killed: bool,
    show_army_lost: bool,
    overlay_font_size: f32,
) -> (f32, f32) {
    let mut column_widths = vec![
        scaled_width(120.0, overlay_font_size, 7.5), // player
    ];

    if show_gold {
        column_widths.push(scaled_width(68.0, overlay_font_size, 4.4));
    }
    if show_population {
        column_widths.push(scaled_width(82.0, overlay_font_size, 5.2));
    }
    if show_army {
        column_widths.push(scaled_width(52.0, overlay_font_size, 3.4));
    }
    if show_food_effect {
        column_widths.push(scaled_width(50.0, overlay_font_size, 3.3));
    }
    if show_tax_effect {
        column_widths.push(scaled_width(50.0, overlay_font_size, 3.3));
    }
    if show_ha {
        column_widths.push(scaled_width(42.0, overlay_font_size, 2.8));
    }
    if show_cl {
        column_widths.push(scaled_width(42.0, overlay_font_size, 2.8));
    }
    if show_shooters {
        column_widths.push(scaled_width(58.0, overlay_font_size, 3.8));
    }
    if show_acq_gold {
        column_widths.push(scaled_width(76.0, overlay_font_size, 4.9));
    }
    if show_prod_food {
        column_widths.push(scaled_width(78.0, overlay_font_size, 5.0));
    }
    if show_prod_weapons {
        column_widths.push(scaled_width(78.0, overlay_font_size, 5.0));
    }
    if show_prod_stone {
        column_widths.push(scaled_width(78.0, overlay_font_size, 5.0));
    }
    if show_prod_iron {
        column_widths.push(scaled_width(76.0, overlay_font_size, 4.9));
    }
    if show_prod_wood {
        column_widths.push(scaled_width(78.0, overlay_font_size, 5.0));
    }
    if show_total_effect {
        column_widths.push(scaled_width(56.0, overlay_font_size, 3.6));
    }
    if show_knights {
        column_widths.push(scaled_width(58.0, overlay_font_size, 3.8));
    }
    if show_ff {
        column_widths.push(scaled_width(42.0, overlay_font_size, 2.8));
    }
    if show_army_killed {
        column_widths.push(scaled_width(82.0, overlay_font_size, 5.2));
    }
    if show_army_lost {
        column_widths.push(scaled_width(72.0, overlay_font_size, 4.6));
    }

    let spacing = 12.0 * (column_widths.len().saturating_sub(1) as f32);
    let horizontal_padding = 28.0;
    let border_allowance = 56.0;
    let width =
        horizontal_padding + border_allowance + column_widths.into_iter().sum::<f32>() + spacing;
    let row_count = visible_player_count.max(1);
    let row_height = (overlay_font_size * 1.9).max(28.0);
    let header_height = (overlay_font_size * 1.05).max(16.0);
    let title_height = (overlay_font_size * 1.25).max(24.0);
    let vertical_padding = 14.0;
    let spacing = 5.0 * (row_count as f32 + 1.0);
    let height =
        vertical_padding + title_height + header_height + row_count as f32 * row_height + spacing;

    (width.max(320.0), height)
}

fn scaled_width(min_width: f32, font_size: f32, multiplier: f32) -> f32 {
    min_width.max(font_size * multiplier)
}

fn cycle_player_color(app: &AppWindow, player_id: i32) {
    let next_color = |color: i32| {
        if color >= 8 {
            1
        } else {
            color + 1
        }
    };

    match player_id {
        1 => app.set_player1_color(next_color(app.get_player1_color())),
        2 => app.set_player2_color(next_color(app.get_player2_color())),
        3 => app.set_player3_color(next_color(app.get_player3_color())),
        4 => app.set_player4_color(next_color(app.get_player4_color())),
        5 => app.set_player5_color(next_color(app.get_player5_color())),
        6 => app.set_player6_color(next_color(app.get_player6_color())),
        7 => app.set_player7_color(next_color(app.get_player7_color())),
        8 => app.set_player8_color(next_color(app.get_player8_color())),
        _ => {}
    }
}

fn player_names(app: &AppWindow) -> [slint::SharedString; 8] {
    [
        app.get_player1_name(),
        app.get_player2_name(),
        app.get_player3_name(),
        app.get_player4_name(),
        app.get_player5_name(),
        app.get_player6_name(),
        app.get_player7_name(),
        app.get_player8_name(),
    ]
}

fn visible_players(app: &AppWindow) -> [bool; 8] {
    [
        app.get_show_player1(),
        app.get_show_player2(),
        app.get_show_player3(),
        app.get_show_player4(),
        app.get_show_player5(),
        app.get_show_player6(),
        app.get_show_player7(),
        app.get_show_player8(),
    ]
}

fn player_colors(app: &AppWindow) -> [i32; 8] {
    [
        app.get_player1_color(),
        app.get_player2_color(),
        app.get_player3_color(),
        app.get_player4_color(),
        app.get_player5_color(),
        app.get_player6_color(),
        app.get_player7_color(),
        app.get_player8_color(),
    ]
}

fn shooter_options(app: &AppWindow) -> ShooterUiOptions {
    ShooterUiOptions {
        count_skirmishers: app.get_shooter_count_skirmishers(),
        count_archers: app.get_shooter_count_archers(),
        count_xbows: app.get_shooter_count_xbows(),
        count_arab_bows: app.get_shooter_count_arab_bows(),
        count_slingers: app.get_shooter_count_slingers(),
    }
}
