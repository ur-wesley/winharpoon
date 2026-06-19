use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::app::AppState;
use crate::apps::favorites::{FavoritesStore, SharedFavorites};
use crate::apps::search::FuzzySearch;
use crate::apps::{entries, is_ready, launch, maybe_refresh, AppEntry, AppIndexRef};
use crate::config::Config;
use crate::hotkeys::post_reload;
use crate::log;
use crate::ui::list::ListSelection;

#[derive(Debug, Clone, Copy, Default)]
pub struct AppMenuAnchor;

#[derive(Debug, Clone)]
pub struct AppMenuRow {
    pub entry: AppEntry,
    pub favorite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMenuAction {
    SetQuery,
    Navigate(i32),
    Hover(Option<usize>),
    Launch(usize),
    ToggleFavorite(String),
    SetFavoriteHotkey { id: String, chord: String },
    CancelHotkeyCapture,
    Close,
}

pub struct AppMenuController {
    pub visible: bool,
    pub anchor: AppMenuAnchor,
    pub query: String,
    pub selection: ListSelection,
    pub panel_width: f32,
    pub panel_height: f32,
    pub rows: Vec<AppMenuRow>,
    pub capture_hotkey_id: Option<String>,
    pub last_query: String,
    pub max_results: usize,
    search: FuzzySearch,
}

impl Default for AppMenuController {
    fn default() -> Self {
        Self {
            visible: false,
            anchor: AppMenuAnchor,
            query: String::new(),
            selection: ListSelection::default(),
            panel_width: 440.0,
            panel_height: 420.0,
            rows: Vec::new(),
            capture_hotkey_id: None,
            last_query: String::new(),
            max_results: 16,
            search: FuzzySearch::default(),
        }
    }
}

impl AppMenuController {
    pub fn on_show(&mut self, anchor: AppMenuAnchor, config: &Config, favorites: &SharedFavorites) {
        log::debug("app menu show");
        self.anchor = anchor;
        self.query.clear();
        self.last_query.clear();
        self.selection.selected = 0;
        self.selection.hovered = None;
        self.capture_hotkey_id = None;
        self.selection.scroll_to_selected = true;
        self.max_results = config.apps.max_results;
        self.panel_width = config.apps.width;
        self.panel_height = config.apps.height;
        self.rebuild_rows(favorites);
        self.visible = true;
        maybe_refresh();
    }

    pub fn panel_size(&self) -> (f32, f32) {
        (self.panel_width, self.panel_height)
    }

    pub fn trailing_count(&self) -> String {
        if is_ready() {
            entries().len().to_string()
        } else {
            "…".into()
        }
    }

    pub fn on_query_changed(&mut self, favorites: &SharedFavorites) {
        if self.query != self.last_query {
            self.last_query = self.query.clone();
            self.selection.reset_on_query_change();
            self.rebuild_rows(favorites);
        }
    }

    pub fn handle_action(
        &mut self,
        action: AppMenuAction,
        favorites: &SharedFavorites,
        state: &Arc<Mutex<AppState>>,
    ) {
        match action {
            AppMenuAction::SetQuery => {
                self.on_query_changed(favorites);
            }
            AppMenuAction::Navigate(delta) => {
                self.selection.navigate(self.rows.len(), delta);
            }
            AppMenuAction::Hover(hovered) => {
                self.selection.hovered = hovered;
            }
            AppMenuAction::Launch(idx) => {
                if let Some(row) = self.rows.get(idx) {
                    let entry = row.entry.clone();
                    std::thread::spawn(move || {
                        let _ = launch(&entry);
                    });
                    self.visible = false;
                }
            }
            AppMenuAction::ToggleFavorite(id) => {
                favorites.lock().toggle(&id);
                self.rebuild_rows(favorites);
                post_reload();
            }

            AppMenuAction::SetFavoriteHotkey { id, chord } => {
                let config_guard = state.lock().config.lock().clone();
                let mut fav_bindings = favorites.lock().hotkey_bindings();
                fav_bindings.retain(|b| b.label != format!("favorite_{id}"));
                if let Ok(parsed) = crate::config::parse_chord(&chord) {
                    fav_bindings.push(crate::config::HotkeyBinding {
                        action: crate::hotkeys::HotkeyAction::LaunchFavorite(0),
                        label: format!("favorite_{id}"),
                        chord: chord.clone(),
                        parsed: Some(parsed),
                    });
                }
                if config_guard.validate_merged(&fav_bindings).is_err() {
                    log::notify("WinHarpoon", "Hotkey conflicts with an existing binding");
                    self.capture_hotkey_id = None;
                    return;
                }
                favorites.lock().set_hotkey(&id, chord);
                self.capture_hotkey_id = None;
                self.rebuild_rows(favorites);
                post_reload();
            }
            AppMenuAction::CancelHotkeyCapture => {
                self.capture_hotkey_id = None;
            }
            AppMenuAction::Close => {
                self.visible = false;
                self.capture_hotkey_id = None;
            }
        }
    }

    fn rebuild_rows(&mut self, favorites: &SharedFavorites) {
        let store = favorites.lock();
        let all = entries();
        self.rows = build_app_rows(
            &self.query,
            self.max_results,
            &all,
            &store,
            &mut self.search,
        );
        clamp_selection(&mut self.selection.selected, self.rows.len());
    }
}

pub(crate) fn build_app_rows(
    query: &str,
    max_results: usize,
    all: &[AppEntry],
    favorites: &FavoritesStore,
    search: &mut FuzzySearch,
) -> Vec<AppMenuRow> {
    let index = AppIndexRef::new(all);
    let fav_set: HashSet<_> = favorites.favorites.iter().map(|f| f.id.as_str()).collect();

    let picked: Vec<AppEntry> = if query.trim().is_empty() {
        let mut out: Vec<AppEntry> = favorites
            .favorites
            .iter()
            .filter_map(|f| index.by_id.get(&f.id).map(|e| (*e).clone()))
            .collect();
        let fav_ids: HashSet<String> = out.iter().map(|e| e.id.clone()).collect();
        for e in all
            .iter()
            .filter(|e| !fav_ids.contains(&e.id))
            .take(max_results)
        {
            out.push(e.clone());
        }
        out
    } else {
        let labels: Vec<_> = all.iter().map(|e| e.search_label.as_str()).collect();
        let mut ranked = search.rank(query, &labels, max_results * 4);
        sort_ranked_favorites_first(&mut ranked, all, &fav_set);
        ranked.truncate(max_results);
        ranked
            .iter()
            .filter_map(|(i, _)| all.get(*i).cloned())
            .collect()
    };

    picked
        .into_iter()
        .map(|e| AppMenuRow {
            favorite: fav_set.contains(e.id.as_str()),
            entry: e,
        })
        .collect()
}

pub(crate) fn sort_ranked_favorites_first(
    ranked: &mut [(usize, u32)],
    all: &[AppEntry],
    fav_set: &HashSet<&str>,
) {
    ranked.sort_by(|a, b| {
        let fav_a = fav_set.contains(all[a.0].id.as_str());
        let fav_b = fav_set.contains(all[b.0].id.as_str());
        fav_b.cmp(&fav_a).then_with(|| b.1.cmp(&a.1))
    });
}

pub(crate) fn clamp_selection(selected: &mut usize, row_count: usize) {
    if *selected >= row_count {
        *selected = 0;
    }
}

pub fn icon_path(entry: &AppEntry) -> &std::path::Path {
    if entry.source_lnk.exists() {
        &entry.source_lnk
    } else {
        &entry.target
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::apps::favorites::FavoriteEntry;

    fn test_entry(id: &str, name: &str, label: &str) -> AppEntry {
        AppEntry {
            id: id.into(),
            name: name.into(),
            target: std::path::PathBuf::from(format!("C:\\{name}.exe")),
            args: String::new(),
            source_lnk: std::path::PathBuf::from(format!("C:\\{name}.lnk")),
            search_label: label.into(),
        }
    }

    #[test]
    fn favorites_listed_first_when_query_empty() {
        let all = vec![
            test_entry("a", "Alpha", "alpha"),
            test_entry("b", "Beta", "beta"),
            test_entry("c", "Gamma", "gamma"),
        ];
        let favorites = FavoritesStore {
            favorites: vec![
                FavoriteEntry {
                    id: "c".into(),
                    hotkey: String::new(),
                },
                FavoriteEntry {
                    id: "a".into(),
                    hotkey: String::new(),
                },
            ],
        };
        let rows = build_app_rows("", 16, &all, &favorites, &mut FuzzySearch::default());
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].entry.id, "c");
        assert_eq!(rows[1].entry.id, "a");
        assert_eq!(rows[2].entry.id, "b");
        assert!(rows[0].favorite);
        assert!(!rows[2].favorite);
    }

    #[test]
    fn sort_ranked_favorites_first_breaks_score_ties() {
        let all = vec![
            test_entry("a", "Alpha", "alpha"),
            test_entry("b", "Beta", "beta"),
        ];
        let fav_set: HashSet<&str> = HashSet::from(["b"]);
        let mut ranked = vec![(0, 100), (1, 100)];
        sort_ranked_favorites_first(&mut ranked, &all, &fav_set);
        assert_eq!(ranked[0].0, 1);
        assert_eq!(ranked[1].0, 0);
    }

    #[test]
    fn clamp_selection_resets_when_out_of_range() {
        let mut selected = 5;
        clamp_selection(&mut selected, 2);
        assert_eq!(selected, 0);
    }

    #[test]
    fn fuzzy_search_favorites_rank_higher() {
        let all = vec![
            test_entry("a", "Chrome", "chrome browser"),
            test_entry("b", "Chromium", "chromium browser"),
        ];
        let favorites = FavoritesStore {
            favorites: vec![FavoriteEntry {
                id: "b".into(),
                hotkey: String::new(),
            }],
        };
        let rows = build_app_rows("chromium", 16, &all, &favorites, &mut FuzzySearch::default());
        assert!(!rows.is_empty());
        assert_eq!(rows[0].entry.id, "b");
    }

    #[test]
    fn fuzzy_search_favorites_always_on_top_regardless_of_score() {
        let all = vec![
            test_entry("a", "Chrome", "chrome browser"),
            test_entry("b", "Chromium", "chromium browser"),
        ];
        let favorites = FavoritesStore {
            favorites: vec![FavoriteEntry {
                id: "b".into(),
                hotkey: String::new(),
            }],
        };
        let rows = build_app_rows("chrome", 16, &all, &favorites, &mut FuzzySearch::default());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].entry.id, "b");
        assert_eq!(rows[1].entry.id, "a");
    }
}
