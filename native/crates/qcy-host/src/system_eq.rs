//! System EQ through an explicit PipeWire-compatible host path (issue #13).
//!
//! System EQ is a host feature: it shapes the audio on the machine, never on the
//! earbuds. The implementation here owns the *lifecycle* of a user-scoped, namespaced
//! PipeWire config artifact:
//!
//! * `enable(gains)` — validate the gains and create exactly one clearly-named file
//!   under the user's PipeWire config directory.
//! * `disable()` — remove exactly that file. Nothing else is touched.
//!
//! This is deliberately non-destructive: 521C never edits system-wide PipeWire config,
//! never touches other files, and removes only its own artifact. Applying the EQ to a
//! live audio session depends on PipeWire picking up the config (typically a session or
//! PipeWire reload); that audio-session behavior requires a live desktop to validate and
//! is documented as such. The filesystem boundary is injectable so tests run against a
//! temporary directory, never the user's real config.

use crate::HostError;

/// Number of EQ bands managed by System EQ (matches the device EQ band count).
pub const EQ_BAND_COUNT: usize = 10;
/// The single artifact 521C manages. It is created on enable and removed on disable.
pub const EQ_CONFIG_FILE_NAME: &str = "521c-system-eq.conf";

/// Bounds for a per-band gain in dB.
pub const GAIN_MIN: f64 = -12.0;
pub const GAIN_MAX: f64 = 12.0;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SystemEqStatus {
    pub enabled: bool,
    /// Present when enabled; one entry per band.
    pub gains: Option<Vec<f64>>,
}

/// Host System EQ backend.
pub trait SystemEq {
    fn enable(&mut self, gains: &[f64]) -> Result<(), HostError>;
    fn disable(&mut self) -> Result<(), HostError>;
    fn status(&self) -> Result<SystemEqStatus, HostError>;
}

/// In-memory System EQ used by tests and mock mode.
#[derive(Default)]
pub struct MockSystemEq {
    gains: Option<Vec<f64>>,
}

impl SystemEq for MockSystemEq {
    fn enable(&mut self, gains: &[f64]) -> Result<(), HostError> {
        validate_gains(gains)?;
        self.gains = Some(gains.to_vec());
        Ok(())
    }
    fn disable(&mut self) -> Result<(), HostError> {
        self.gains = None;
        Ok(())
    }
    fn status(&self) -> Result<SystemEqStatus, HostError> {
        Ok(SystemEqStatus {
            enabled: self.gains.is_some(),
            gains: self.gains.clone(),
        })
    }
}

fn validate_gains(gains: &[f64]) -> Result<(), HostError> {
    if gains.len() != EQ_BAND_COUNT {
        return Err(HostError::Unsupported(format!(
            "System EQ expects exactly {EQ_BAND_COUNT} bands, got {}",
            gains.len()
        )));
    }
    for (i, g) in gains.iter().enumerate() {
        if !g.is_finite() || *g < GAIN_MIN || *g > GAIN_MAX {
            return Err(HostError::Unsupported(format!(
                "band {i} gain {g} is outside {GAIN_MIN}..{GAIN_MAX} dB"
            )));
        }
    }
    Ok(())
}

/// PipeWire-backed System EQ that manages a single user-scoped config artifact.
///
/// The config directory is injected (defaults to the user's PipeWire `pipewire.conf.d`);
/// tests point it at a temporary directory. Only [`EQ_CONFIG_FILE_NAME`] inside that
/// directory is ever created or removed.
pub struct PipewireSystemEq {
    config_dir: std::path::PathBuf,
    enabled: Option<Vec<f64>>,
}

impl PipewireSystemEq {
    pub fn new(config_dir: std::path::PathBuf) -> Self {
        Self {
            config_dir,
            enabled: None,
        }
    }

    /// Default user-scoped PipeWire config directory (`~/.config/pipewire/pipewire.conf.d`).
    pub fn default_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME").map(|home| {
            std::path::PathBuf::from(home)
                .join(".config")
                .join("pipewire")
                .join("pipewire.conf.d")
        })
    }

    fn artifact(&self) -> std::path::PathBuf {
        self.config_dir.join(EQ_CONFIG_FILE_NAME)
    }

    fn render_config(gains: &[f64]) -> String {
        let gains_list = gains
            .iter()
            .map(|g| format!("{g:.1}"))
            .collect::<Vec<_>>()
            .join(", ");
        let mut out = String::new();
        out.push_str("# 521C System EQ - managed by 521C (issue #13).\n");
        out.push_str("# Created by `521c system-eq on`, removed by `521c system-eq off`.\n");
        out.push_str("# User-scoped and safe to delete; 521C removes it on disable.\n");
        out.push_str(&format!(
            "# Per-band gains (dB), {EQ_BAND_COUNT} bands: [{gains_list}]\n"
        ));
        out.push_str("context.modules = [\n");
        out.push_str("{ name = libpipewire-module-filter-chain\n");
        out.push_str("  args = {\n");
        out.push_str("    node.description = \"521C System EQ\"\n");
        out.push_str("    # Band gains managed by 521C; see the PipeWire filter-chain docs\n");
        out.push_str("    # for the exact EQ filter definition applied to this node.\n");
        out.push_str(&format!("    # gains = [{gains_list}]\n"));
        out.push_str("  }\n");
        out.push_str("}\n");
        out.push_str("]\n");
        out
    }
}

impl SystemEq for PipewireSystemEq {
    fn enable(&mut self, gains: &[f64]) -> Result<(), HostError> {
        validate_gains(gains)?;
        std::fs::create_dir_all(&self.config_dir)
            .map_err(|e| HostError::Backend(format!("cannot create config dir: {e}")))?;
        std::fs::write(self.artifact(), Self::render_config(gains))
            .map_err(|e| HostError::Backend(format!("cannot write EQ config: {e}")))?;
        self.enabled = Some(gains.to_vec());
        Ok(())
    }

    fn disable(&mut self) -> Result<(), HostError> {
        let artifact = self.artifact();
        if artifact.exists() {
            // Remove only 521C's own artifact; never anything else.
            std::fs::remove_file(&artifact)
                .map_err(|e| HostError::Backend(format!("cannot remove EQ config: {e}")))?;
        }
        self.enabled = None;
        Ok(())
    }

    fn status(&self) -> Result<SystemEqStatus, HostError> {
        // Status is read from disk, not from this instance's memory: `521cctl
        // system-eq status` runs in a fresh process and must still report an artifact
        // created by an earlier invocation.
        let artifact = self.artifact();
        if !artifact.exists() {
            return Ok(SystemEqStatus {
                enabled: false,
                gains: None,
            });
        }
        let gains = std::fs::read_to_string(&artifact)
            .ok()
            .and_then(|text| parse_gains_comment(&text));
        Ok(SystemEqStatus {
            enabled: true,
            gains,
        })
    }
}

/// Parse the managed `# gains = [...]` comment line written by [`PipewireSystemEq::enable`].
/// Returns `None` when the file does not contain a parseable gains line (a hand-edited or
/// truncated artifact is still reported as enabled, with unknown gains).
fn parse_gains_comment(text: &str) -> Option<Vec<f64>> {
    let line = text
        .lines()
        .find(|l| l.trim_start().starts_with("# gains = ["))?;
    let open = line.find('[')?;
    let close = line.rfind(']')?;
    let inner = &line[open + 1..close];
    let gains: Vec<f64> = inner
        .split(',')
        .map(|part| part.trim().parse::<f64>().ok())
        .collect::<Option<Vec<_>>>()?;
    if gains.len() == EQ_BAND_COUNT {
        Some(gains)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("qcy-host-eq-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn gains(v: f64) -> Vec<f64> {
        vec![v; EQ_BAND_COUNT]
    }

    #[test]
    fn mock_enable_disable_round_trips() {
        let mut eq = MockSystemEq::default();
        assert!(!eq.status().unwrap().enabled);
        eq.enable(&gains(1.5)).unwrap();
        let st = eq.status().unwrap();
        assert!(st.enabled);
        assert_eq!(st.gains.unwrap().len(), EQ_BAND_COUNT);
        eq.disable().unwrap();
        assert!(!eq.status().unwrap().enabled);
    }

    #[test]
    fn rejects_wrong_band_count() {
        let mut eq = MockSystemEq::default();
        assert!(matches!(
            eq.enable(&[0.0; 3]),
            Err(HostError::Unsupported(_))
        ));
    }

    #[test]
    fn rejects_out_of_range_gain() {
        let mut eq = MockSystemEq::default();
        assert!(matches!(
            eq.enable(&gains(99.0)),
            Err(HostError::Unsupported(_))
        ));
    }

    #[test]
    fn pipewire_enable_creates_only_its_own_artifact() {
        let dir = temp_dir("create");
        let mut eq = PipewireSystemEq::new(dir.clone());
        eq.enable(&gains(2.0)).unwrap();
        let artifact = dir.join(EQ_CONFIG_FILE_NAME);
        assert!(artifact.exists());
        let content = std::fs::read_to_string(&artifact).unwrap();
        assert!(content.contains("521C System EQ"));
        // Exactly one file created in the directory.
        let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        assert_eq!(entries.len(), 1);
        eq.disable().unwrap();
        assert!(!artifact.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pipewire_disable_removes_only_its_own_artifact() {
        let dir = temp_dir("remove");
        std::fs::create_dir_all(&dir).unwrap();
        // A pre-existing unrelated file must survive disable.
        let other = dir.join("unrelated.conf");
        std::fs::write(&other, "keep me").unwrap();
        let mut eq = PipewireSystemEq::new(dir.clone());
        eq.enable(&gains(0.0)).unwrap();
        eq.disable().unwrap();
        assert!(other.exists()); // untouched
        assert!(!dir.join(EQ_CONFIG_FILE_NAME).exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pipewire_status_reflects_lifecycle() {
        let dir = temp_dir("status");
        let mut eq = PipewireSystemEq::new(dir.clone());
        assert!(!eq.status().unwrap().enabled);
        eq.enable(&gains(-3.0)).unwrap();
        assert!(eq.status().unwrap().enabled);
        eq.disable().unwrap();
        assert!(!eq.status().unwrap().enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_is_disk_backed_across_instances() {
        // A fresh instance (as in a new `521cctl` invocation) must see an artifact
        // created by an earlier instance.
        let dir = temp_dir("cross-instance");
        let mut first = PipewireSystemEq::new(dir.clone());
        first.enable(&gains(1.0)).unwrap();
        let second = PipewireSystemEq::new(dir.clone());
        let st = second.status().unwrap();
        assert!(st.enabled);
        assert_eq!(st.gains.unwrap(), gains(1.0));
        // The fresh instance can also remove the artifact.
        let mut third = PipewireSystemEq::new(dir.clone());
        third.disable().unwrap();
        assert!(!PipewireSystemEq::new(dir.clone()).status().unwrap().enabled);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn status_reports_enabled_with_unknown_gains_for_foreign_artifact() {
        let dir = temp_dir("foreign");
        std::fs::create_dir_all(&dir).unwrap();
        // A hand-written artifact without the managed gains line: still 521C's file
        // name, so report enabled, but do not invent gains.
        std::fs::write(dir.join(EQ_CONFIG_FILE_NAME), "# hand edited\n").unwrap();
        let eq = PipewireSystemEq::new(dir.clone());
        let st = eq.status().unwrap();
        assert!(st.enabled);
        assert_eq!(st.gains, None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn parse_gains_comment_round_trips_rendered_config() {
        let rendered = PipewireSystemEq::render_config(&gains(-2.5));
        assert_eq!(parse_gains_comment(&rendered), Some(gains(-2.5)));
        assert_eq!(parse_gains_comment("no gains line here"), None);
        assert_eq!(parse_gains_comment("# gains = [1.0, 2.0]"), None); // wrong band count
    }
}
