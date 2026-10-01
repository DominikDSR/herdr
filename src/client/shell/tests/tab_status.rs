use super::*;
use crate::client::shell::render::{display_width, render_tab_bar};

fn render_tabs(
    projected: &ClientShellSnapshot,
    config: &ClientShellConfig,
    width: u16,
) -> (Buffer, ShellHitMap, usize) {
    let area = Rect::new(3, 2, width, 1);
    let mut buffer = Buffer::empty(area);
    let mut hits = ShellHitMap::default();
    let mut scroll = 0;
    let mut reveal = true;
    render_tab_bar(
        &mut buffer,
        area,
        projected,
        config,
        &mut scroll,
        &mut reveal,
        None,
        &mut hits,
    );
    (buffer, hits, scroll)
}

fn tab_text(buffer: &Buffer, rect: Rect) -> String {
    let mut text = String::new();
    let mut x = rect.x;
    while x < rect.right() {
        let symbol = buffer.cell((x, rect.y)).unwrap().symbol();
        text.push_str(symbol);
        x += display_width(symbol).max(1);
    }
    text
}

#[test]
fn tab_status_is_opt_in_and_does_not_change_stored_names() {
    let projected = snapshot();
    let mut config = ClientShellConfig::from_config(&Config::default());
    let (plain, plain_hits, _) = render_tabs(&projected, &config, 80);
    assert_eq!(tab_text(&plain, plain_hits.tabs[0].0).trim(), "1");

    config.show_agent_status_on_tabs = true;
    let (marked, marked_hits, _) = render_tabs(&projected, &config, 80);
    assert_eq!(tab_text(&marked, marked_hits.tabs[0].0).trim(), "○ 1");
    assert_eq!(projected.tabs[0].label, "1");
    assert!(!projected.tabs[0].custom_label);
    assert_eq!(marked_hits.tabs[0].1, plain_hits.tabs[0].1);

    config.show_agent_status_on_tabs = false;
    let (restored, restored_hits, _) = render_tabs(&projected, &config, 80);
    assert_eq!(restored, plain);
    assert_eq!(restored_hits.tabs, plain_hits.tabs);
}

#[test]
fn tab_status_reuses_sidebar_icons_and_colors_for_every_state() {
    use crate::config::StatusIndicatorStyle;
    for theme in ["catppuccin", "catppuccin-latte"] {
        let mut source = Config::default();
        source.theme.name = Some(theme.into());
        source.ui.show_agent_status_on_tabs = true;
        for indicators in [StatusIndicatorStyle::Dots, StatusIndicatorStyle::Symbols] {
            source.ui.status_indicators = indicators;
            let config = ClientShellConfig::from_config(&source);
            for focused in [false, true] {
                for status in [
                    AgentStatus::Blocked,
                    AgentStatus::Done,
                    AgentStatus::Working,
                    AgentStatus::Idle,
                    AgentStatus::Unknown,
                ] {
                    let mut projected = snapshot();
                    projected.tabs[0].agent_status = status;
                    projected.tabs[0].focused = focused;
                    projected.tabs[0].label = "backend".into();
                    let (buffer, hits, _) = render_tabs(&projected, &config, 80);
                    let rect = hits.tabs[0].0;
                    assert_eq!(rect.width, display_width("● backend") + 4);
                    let icon = buffer.cell((rect.x + 2, rect.y)).unwrap();
                    assert_eq!(icon.symbol(), status_icon(status, indicators));
                    assert_eq!(icon.fg, status_color(status, &config.palette));
                    assert_eq!(icon.bg, config.palette.surface0);
                    assert!(!icon.modifier.contains(Modifier::DIM));
                    let label = buffer.cell((rect.x + 4, rect.y)).unwrap();
                    assert_eq!(label.symbol(), "b");
                    assert_eq!(
                        label.bg,
                        if focused {
                            config.palette.accent
                        } else {
                            config.palette.surface0
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn tab_status_uses_the_server_rollup_not_the_focused_pane() {
    let mut projected = snapshot();
    projected.tabs[0].agent_status = AgentStatus::Blocked;
    // No agent records are needed to render the server-owned tab rollup.
    assert!(projected.agents.is_empty());
    let mut source = Config::default();
    source.ui.show_agent_status_on_tabs = true;
    source.ui.status_indicators = crate::config::StatusIndicatorStyle::Symbols;
    let config = ClientShellConfig::from_config(&source);
    let (buffer, hits, _) = render_tabs(&projected, &config, 80);
    assert_eq!(tab_text(&buffer, hits.tabs[0].0).trim(), "× 1");
    projected.tabs[0].agent_status = AgentStatus::Idle;
    let (buffer, hits, _) = render_tabs(&projected, &config, 80);
    assert_eq!(tab_text(&buffer, hits.tabs[0].0).trim(), "○ 1");
}

#[test]
fn tab_status_preserves_unicode_labels_zoom_and_hit_geometry() {
    let mut projected = snapshot();
    projected.tabs[0].label = "日本".into();
    projected.tabs[0].custom_label = true;
    projected.tabs[0].zoomed = true;
    projected.tabs.push(ClientShellTab {
        tab_id: "other-space".into(),
        workspace_id: "ws_2".into(),
        ..projected.tabs[0].clone()
    });
    let mut config = ClientShellConfig::from_config(&Config::default());
    config.show_agent_status_on_tabs = true;
    let (buffer, hits, _) = render_tabs(&projected, &config, 80);
    assert_eq!(hits.tabs.len(), 1);
    let rect = hits.tabs[0].0;
    assert_eq!(rect.width, display_width("○ 日本 Z") + 4);
    assert!(tab_text(&buffer, rect).contains("○ 日本 Z"));
    assert_eq!(hits.new_tab.x, rect.right() + 1);
}

#[test]
fn tab_status_clips_safely_and_keeps_overflow_tabs_reachable() {
    let mut projected = snapshot();
    for number in 2..=15 {
        projected.tabs.push(ClientShellTab {
            tab_id: format!("tab_{number}"),
            number,
            label: format!("backend-{number}"),
            focused: number == 15,
            ..projected.tabs[0].clone()
        });
    }
    projected.tabs[0].focused = false;
    let mut config = ClientShellConfig::from_config(&Config::default());
    config.show_agent_status_on_tabs = true;
    for mouse in [false, true] {
        config.mouse_capture = mouse;
        for width in 0..=80 {
            let (buffer, hits, scroll) = render_tabs(&projected, &config, width);
            for (rect, _) in &hits.tabs {
                assert!(rect.x >= buffer.area.x);
                assert!(rect.right() <= buffer.area.right());
            }
            assert!(scroll < projected.tabs.len());
            if width >= 30 {
                assert!(hits.tabs.iter().any(|(_, id)| id == "tab_15"));
            }
        }
    }
}

#[test]
fn tab_status_renders_with_sidebar_hidden_and_bottom_tab_bar() {
    let mut config = Config::default();
    config.ui.show_agent_status_on_tabs = true;
    config.ui.sidebar_start_collapsed = true;
    config.ui.sidebar_collapsed_mode = SidebarCollapsedModeConfig::Hidden;
    config.ui.tab_bar_position = TabBarPositionConfig::Bottom;
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&config));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    let frame = state.compose(106, 20).expect("bottom tab status frame");
    let rect = state.hits.tabs[0].0;
    assert_eq!(rect.y, 19);
    let buffer = frame.to_ratatui_buffer().unwrap();
    assert_eq!(tab_text(&buffer, rect).trim(), "○ 1");

    config.ui.hide_tab_bar_when_single_tab = true;
    state.config.apply_live_config(&config, &[], &[]);
    state.compose(106, 20).expect("hidden single-tab row");
    assert!(state.hits.tabs.is_empty());
}

#[test]
#[ignore = "non-gating fixed-geometry tab status composition profile"]
fn tab_status_render_scale_profile() {
    for count in [1, 15] {
        let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
        state.set_snapshot(Box::new(snapshot()));
        let mut next = surface();
        next.frame =
            FrameData::from_ratatui_buffer(&Buffer::with_lines(vec!["x".repeat(120); 30]), None);
        let template = next.panes[0].clone();
        next.panes.clear();
        for index in 0..count {
            let mut pane = template.clone();
            pane.pane_id = format!("pane_{index}");
            pane.inner_rect = SurfaceRect {
                x: if count == 1 { 0 } else { (index % 5) * 24 },
                y: if count == 1 { 0 } else { (index / 5) * 10 },
                width: if count == 1 { 120 } else { 24 },
                height: if count == 1 { 30 } else { 10 },
            };
            pane.rect = pane.inner_rect;
            next.panes.push(pane);
        }
        state.set_pane_surface(next);
        for enabled in [false, true] {
            state.config.show_agent_status_on_tabs = enabled;
            state.compose(146, 32).unwrap();
            let mut samples = Vec::new();
            for _ in 0..200 {
                let started = std::time::Instant::now();
                std::hint::black_box(state.compose(146, 32).unwrap());
                samples.push(started.elapsed().as_micros());
            }
            samples.sort_unstable();
            eprintln!(
                "tab-status panes={count} enabled={enabled} median_us={} p95_us={}",
                samples[100], samples[190]
            );
            assert!(state.pending_requests.is_empty());
        }
    }
}

#[test]
fn tab_status_setting_is_accessible_and_reflects_current_value() {
    let mut state = ClientShellState::new(ClientShellConfig::from_config(&Config::default()));
    state.set_snapshot(Box::new(snapshot()));
    state.set_pane_surface(surface());
    state.open_settings_overlay();
    state.select_settings_section(
        ClientSettingsSection::Tabs,
        &mut ClientShellInput::default(),
    );
    let frame = state.compose(106, 30).expect("tab status settings frame");
    assert!(state
        .hits
        .settings_tabs
        .iter()
        .any(|(_, section)| *section == ClientSettingsSection::Tabs));
    assert_eq!(state.hits.settings_choices.len(), 2);
    let text = frame
        .cells
        .iter()
        .map(|cell| cell.symbol.as_str())
        .collect::<String>();
    assert!(text.contains("show agent status on tabs"));
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Settings(ClientSettingsOverlay {
            selected: 1,
            ..
        }))
    ));

    state.config.show_agent_status_on_tabs = true;
    state.select_settings_section(
        ClientSettingsSection::Tabs,
        &mut ClientShellInput::default(),
    );
    assert!(matches!(
        state.overlay,
        Some(ClientShellOverlay::Settings(ClientSettingsOverlay {
            selected: 0,
            ..
        }))
    ));
}
