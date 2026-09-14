use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::log;
use crate::paths;

use super::enumerate;
use super::AppEntry;

#[derive(Debug, Serialize, Deserialize)]
struct AppsCacheFile {
    #[serde(default)]
    schema: u32,
    entries: Vec<AppEntry>,
}

/// Bumped whenever entry identity changes so stale caches are rescanned
/// instead of showing duplicates from an older scheme.
/// v2 adds validation (absolute exe target, non-empty id) to blunt cache poisoning.
const CACHE_SCHEMA: u32 = 2;

pub fn load_or_scan() -> Vec<AppEntry> {
    let path = paths::apps_cache_path();
    if path.exists() {
        if let Ok(text) = fs::read_to_string(&path) {
            if let Some(entries) = parse_cache(&text) {
                log::debug(format!("apps: loaded {} from cache", entries.len()));
                return entries;
            }
            // Security: don't trust a tampered cache — rescan instead.
            log::warn("apps: cache invalid, rescanning");
        }
    }
    let entries = enumerate::scan_all();
    save_cache(&path, &entries);
    entries
}

/// Returns cached entries, or `None` when the cache is missing, unreadable,
/// empty, or was written by an older schema.
fn parse_cache(text: &str) -> Option<Vec<AppEntry>> {
    let cache: AppsCacheFile = toml::from_str(text).ok()?;
    if cache.schema != CACHE_SCHEMA || cache.entries.is_empty() {
        return None;
    }
    // Validate: reject injected entries (relative targets, empty ids, non-exe).
    let valid: Vec<AppEntry> = cache
        .entries
        .into_iter()
        .filter(|e| {
            !e.id.trim().is_empty()
                && !e.name.trim().is_empty()
                && (e.aumid.as_ref().is_some_and(|a| !a.trim().is_empty())
                    || is_plausible_target(&e.target))
        })
        .collect();
    if valid.is_empty() {
        return None;
    }
    Some(valid)
}

fn is_plausible_target(target: &std::path::Path) -> bool {
    if target.as_os_str().is_empty() || !target.is_absolute() {
        return false;
    }
    matches!(
        target
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "exe" | "msi" | "bat" | "cmd" | "lnk"
    )
}

pub fn save_cache(path: &Path, entries: &[AppEntry]) {
    #[cfg(test)]
    {
        let _ = (path, entries);
    }
    #[cfg(not(test))]
    {
        paths::ensure_app_data();
        let file = AppsCacheFile {
            schema: CACHE_SCHEMA,
            entries: entries.to_vec(),
        };
        if let Ok(text) = toml::to_string(&file) {
            let _ = paths::atomic_write(path, &text);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache_toml(schema_line: &str) -> String {
        format!(
            "{schema_line}\n\
             [[entries]]\n\
             id = \"abc\"\n\
             name = \"Test\"\n\
             target = 'C:\\test.exe'\n\
             args = \"\"\n\
             source_lnk = \"\"\n\
             search_label = \"Test\"\n"
        )
    }

    #[test]
    fn accepts_current_schema() {
        let entries = parse_cache(&cache_toml("schema = 2"));
        assert_eq!(entries.map(|e| e.len()), Some(1));
    }

    #[test]
    fn rejects_legacy_cache_without_schema() {
        assert!(parse_cache(&cache_toml("")).is_none());
    }

    #[test]
    fn rejects_wrong_schema_and_empty_entries() {
        assert!(parse_cache(&cache_toml("schema = 999")).is_none());
        assert!(parse_cache(&cache_toml("schema = 1")).is_none());
        let empty = "schema = 2\n";
        assert!(parse_cache(empty).is_none());
    }

    #[test]
    fn rejects_injected_relative_target() {
        let toml = "schema = 2\n\
             [[entries]]\n\
             id = \"evil\"\n\
             name = \"Evil\"\n\
             target = 'evil.exe'\n\
             args = \"/c payload\"\n\
             source_lnk = \"\"\n\
             search_label = \"Evil\"\n";
        assert!(parse_cache(toml).is_none());
    }

    #[test]
    fn rejects_empty_id_and_accepts_aumid() {
        let empty_id = "schema = 2\n[[entries]]\nid = \"\"\nname = \"T\"\ntarget = 'C:\\a.exe'\nargs = \"\"\nsource_lnk = \"\"\nsearch_label = \"T\"\n";
        assert!(parse_cache(empty_id).is_none());
        let aumid = "schema = 2\n[[entries]]\nid = \"x\"\nname = \"Store\"\ntarget = ''\nargs = \"\"\nsource_lnk = \"\"\nsearch_label = \"S\"\naumid = \"Microsoft.App!x\"\n";
        assert_eq!(parse_cache(aumid).map(|e| e.len()), Some(1));
    }
}
