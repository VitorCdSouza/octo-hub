//! The settings the app last wrote to the mouse. The X6 cannot report its own
//! configuration, so this file is the only record of it.

use crate::proto::{self, Action, Light, Polling, Sensor};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub sensor: Sensor,
    pub light: Light,
    pub polling: Polling,
    pub buttons: [Action; 7],
}

impl Default for Profile {
    fn default() -> Self {
        Profile {
            sensor: Sensor::default(),
            light: Light::default(),
            polling: Polling::Hz1000,
            buttons: proto::DEFAULT_BUTTONS,
        }
    }
}

/// Wrapper on disk; `written` is false until the app has sent every report
/// once, because until then the mouse may hold anything.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Saved {
    pub profile: Profile,
    pub written: bool,
}

impl Profile {
    /// Every report, in the order the vendor app sends them at startup.
    pub fn reports(&self) -> Vec<Vec<u8>> {
        vec![
            self.sensor.encode().to_vec(),
            self.light.encode().to_vec(),
            self.polling.encode().to_vec(),
            proto::encode_buttons(&self.buttons).to_vec(),
        ]
    }
}

pub fn config_dir() -> PathBuf {
    if let Some(d) = std::env::var_os("XDG_CONFIG_HOME").filter(|d| !d.is_empty()) {
        return PathBuf::from(d).join("remora");
    }
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join(".config/remora")
}

pub fn default_path() -> PathBuf {
    config_dir().join("mouse.json")
}

pub fn load(path: &Path) -> Result<Saved> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s).with_context(|| format!("could not parse {}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Saved::default()),
        Err(e) => Err(e).with_context(|| format!("could not read {}", path.display())),
    }
}

/// Writes through a temporary file so a crash never leaves half a profile.
pub fn save(path: &Path, saved: &Saved) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("could not create {}", dir.display()))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string_pretty(saved)?).with_context(|| format!("could not write {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("could not replace {}", path.display()))?;
    Ok(())
}

/// Reads a profile exported with `save_export`.
pub fn load_export(path: &Path) -> Result<Profile> {
    let s = std::fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    serde_json::from_str(&s).with_context(|| format!("could not parse {}", path.display()))
}

pub fn save_export(path: &Path, p: &Profile) -> Result<()> {
    std::fs::write(path, serde_json::to_string_pretty(p)?).with_context(|| format!("could not write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_survives_a_round_trip_and_missing_fields() {
        let mut p = Profile::default();
        p.sensor.stages[0].dpi = 1950;
        p.buttons[5] = Action::shortcut(proto::MOD_CTRL, 0x06);
        let s = serde_json::to_string(&Saved { profile: p.clone(), written: true }).unwrap();
        let back: Saved = serde_json::from_str(&s).unwrap();
        assert_eq!(back.profile, p);
        let partial: Saved = serde_json::from_str(r#"{"profile":{"polling":"Hz500"}}"#).unwrap();
        assert_eq!(partial.profile.polling, Polling::Hz500);
        assert_eq!(partial.profile.sensor, Sensor::default());
        assert!(!partial.written);
    }
}
