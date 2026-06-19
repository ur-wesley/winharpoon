use std::sync::Arc;

use parking_lot::Mutex;

use crate::app::request_reload;
use crate::config::{Config, ConfigValidationError};

pub struct SettingsService;

impl SettingsService {
    pub fn persist_apps(draft: &Config, config: &Arc<Mutex<Config>>) -> Result<String, String> {
        draft.save().map_err(|e| e.to_string())?;
        *config.lock() = draft.clone();
        request_reload();
        Ok("Apps settings saved.".into())
    }

    pub fn persist_general(draft: &Config, config: &Arc<Mutex<Config>>) -> Result<String, String> {
        draft.save().map_err(|e| e.to_string())?;
        *config.lock() = draft.clone();
        Ok(if draft.general.autostart {
            "Autostart enabled.".into()
        } else {
            "Autostart disabled.".into()
        })
    }

    pub fn persist_and_reload(
        draft: &Config,
        config: &Arc<Mutex<Config>>,
    ) -> Result<(String, Vec<ConfigValidationError>), String> {
        draft.save().map_err(|e| e.to_string())?;
        *config.lock() = draft.clone();
        request_reload();
        match draft.validate() {
            Ok(_) => Ok(("Binding saved and reloaded.".into(), Vec::new())),
            Err(errors) => Ok((
                "Saved and reloaded with validation warnings.".into(),
                errors,
            )),
        }
    }

    pub fn apply_autostart(enabled: bool) -> Result<(), String> {
        crate::autostart::apply(enabled).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn validate_reports_duplicate_bindings() {
        let mut config = Config::default();
        config.hotkeys.launcher = "Ctrl+Space".into();
        config.hotkeys.same_app_next = "Ctrl+Space".into();
        let errors = config.validate().unwrap_err();
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| matches!(
            e,
            ConfigValidationError::DuplicateBinding { .. }
        )));
    }

    #[test]
    fn validate_accepts_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
    }
}
