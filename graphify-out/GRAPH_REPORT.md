# Graph Report - winharpoon  (2026-06-17)

## Corpus Check
- 29 files · ~14,464 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 354 nodes · 883 edges · 14 communities
- Extraction: 64% EXTRACTED · 36% INFERRED · 0% AMBIGUOUS · INFERRED: 319 edges (avg confidence: 0.8)
- Token cost: 0 input · 0 output

## Community Hubs (Navigation)
- [[_COMMUNITY_Community 0|Community 0]]
- [[_COMMUNITY_Community 1|Community 1]]
- [[_COMMUNITY_Community 2|Community 2]]
- [[_COMMUNITY_Community 3|Community 3]]
- [[_COMMUNITY_Community 4|Community 4]]
- [[_COMMUNITY_Community 5|Community 5]]
- [[_COMMUNITY_Community 6|Community 6]]
- [[_COMMUNITY_Community 7|Community 7]]
- [[_COMMUNITY_Community 8|Community 8]]
- [[_COMMUNITY_Community 9|Community 9]]
- [[_COMMUNITY_Community 10|Community 10]]

## God Nodes (most connected - your core abstractions)
1. `debug()` - 75 edges
2. `warn()` - 26 edges
3. `main()` - 24 edges
4. `SettingsPanel` - 16 edges
5. `LauncherPanel` - 13 edges
6. `dispatch_key()` - 12 edges
7. `cancel_active()` - 12 edges
8. `activate()` - 12 edges
9. `keyboard_proc()` - 12 edges
10. `run_ui()` - 11 edges

## Surprising Connections (you probably didn't know these)
- `main()` --calls--> `take_reload_request()`  [INFERRED]
  src/main.rs → src/app.rs
- `send_ui()` --calls--> `ui_context()`  [INFERRED]
  src/marks_switcher/hook.rs → src/launcher/mod.rs
- `send_ui()` --calls--> `ui_sender()`  [INFERRED]
  src/marks_switcher/hook.rs → src/marks_switcher/mod.rs
- `reload_hotkeys()` --calls--> `debug()`  [INFERRED]
  src/app.rs → src/log.rs
- `reload_hotkeys()` --calls--> `reload_hook()`  [INFERRED]
  src/app.rs → src/marks_switcher/mod.rs

## Communities (14 total, 0 thin omitted)

### Community 0 - "Community 0"
Cohesion: 0.07
Nodes (28): apply_launcher_theme(), LauncherApp, MarksSwitcherPanel, open(), binding_display_name(), BindingSection, egui_key_to_vk(), run_settings() (+20 more)

### Community 1 - "Community 1"
Cohesion: 0.1
Nodes (47): activate(), cancel_active(), cancel_if_active(), confirm_active(), cycle(), dispatch_jump(), dispatch_key(), extra_mods_pressed() (+39 more)

### Community 2 - "Community 2"
Cohesion: 0.11
Nodes (17): SettingsPanel, AppState, open_settings(), request_reload(), take_reload_request(), create_message_window(), HotkeyAction, HotkeyManager (+9 more)

### Community 3 - "Community 3"
Cohesion: 0.11
Nodes (20): badge(), card_frame(), hint_bar(), LauncherPanel, notify_toggle_result(), panel_frame(), search_frame(), UiApp (+12 more)

### Community 4 - "Community 4"
Cohesion: 0.1
Nodes (24): binding(), chord_from_vk_mods(), Config, ConfigValidationError, default_bindings_are_unique_and_parseable(), default_mark_toggle(), default_marks_switcher(), format_modifiers() (+16 more)

### Community 5 - "Community 5"
Cohesion: 0.14
Nodes (21): ensure_installed(), install_hook(), MarksStore, dispatch_action(), reload_hotkeys(), report_config_errors(), error(), info() (+13 more)

### Community 6 - "Community 6"
Cohesion: 0.1
Nodes (23): run_launcher(), filled_entries(), index_after_foreground(), MarkEntry, MarksState, shared_marks(), switcher_entries(), ToggleMarkResult (+15 more)

### Community 7 - "Community 7"
Cohesion: 0.11
Nodes (19): init(), open(), open_settings(), open_tray_menu(), register_context(), signal(), take_pending_launcher(), take_pending_settings() (+11 more)

### Community 8 - "Community 8"
Cohesion: 0.19
Nodes (16): apply(), delete_run_value(), is_enabled(), open_run_key(), quote_exe_path(), read_run_value(), sync_from_config(), write_run_value() (+8 more)

### Community 9 - "Community 9"
Cohesion: 0.21
Nodes (7): tray_menu_divider(), tray_menu_frame(), tray_menu_section_label(), tray_menu_item(), tray_slot_item(), TrayAction, TrayMenuPanel

### Community 10 - "Community 10"
Cohesion: 0.2
Nodes (9): Build, Caveats, code:powershell (cd C:\Arbeit\winharpoon), code:powershell (.\scripts\build-installer.ps1), Config, Default hotkeys, Features, Install (+1 more)

## Knowledge Gaps
- **26 isolated node(s):** `HotkeysConfig`, `LauncherConfig`, `ParsedHotkey`, `HotkeyBinding`, `ConfigValidationError` (+21 more)
  These have ≤1 connection - possible missing edges or undocumented components.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `debug()` connect `Community 2` to `Community 0`, `Community 1`, `Community 3`, `Community 4`, `Community 5`, `Community 6`, `Community 7`, `Community 8`, `Community 9`?**
  _High betweenness centrality (0.539) - this node is a cross-community bridge._
- **Why does `binding()` connect `Community 4` to `Community 2`?**
  _High betweenness centrality (0.036) - this node is a cross-community bridge._
- **Why does `activate()` connect `Community 1` to `Community 2`, `Community 5`, `Community 6`?**
  _High betweenness centrality (0.031) - this node is a cross-community bridge._
- **Are the 72 inferred relationships involving `debug()` (e.g. with `.new()` and `request_reload()`) actually correct?**
  _`debug()` has 72 INFERRED edges - model-reasoned connections that need verification._
- **Are the 22 inferred relationships involving `warn()` (e.g. with `reload_hotkeys()` and `dispatch_action()`) actually correct?**
  _`warn()` has 22 INFERRED edges - model-reasoned connections that need verification._
- **Are the 21 inferred relationships involving `main()` (e.g. with `ensure_app_data()` and `init_toast()`) actually correct?**
  _`main()` has 21 INFERRED edges - model-reasoned connections that need verification._
- **What connects `HotkeysConfig`, `LauncherConfig`, `ParsedHotkey` to the rest of the system?**
  _26 weakly-connected nodes found - possible documentation gaps or missing edges._