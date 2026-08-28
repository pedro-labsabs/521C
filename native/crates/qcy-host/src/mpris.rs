//! MPRIS media discovery, state and control (issue #13).
//!
//! MPRIS players live on the session bus as `org.mpris.MediaPlayer2.<name>` at object
//! path `/org/mpris/MediaPlayer2`. This module never shells out; it uses the D-Bus
//! interface directly. All D-Bus access is behind the [`MprisBus`] trait so discovery,
//! state mapping and control are unit-tested against a fake bus.

use crate::HostError;

/// A media control action. These map 1:1 to `org.mpris.MediaPlayer2.Player` methods.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaAction {
    Play,
    Pause,
    Next,
    Previous,
}

impl MediaAction {
    fn method(self) -> &'static str {
        match self {
            MediaAction::Play => "Play",
            MediaAction::Pause => "Pause",
            MediaAction::Next => "Next",
            MediaAction::Previous => "Previous",
        }
    }
}

/// Normalized media state, shaped for direct use by a UI/CLI. Mirrors the web
/// `MediaState` so the two surfaces stay consistent.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaStatus {
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub player: String,
    /// 0-100; `None` when the player does not expose a volume.
    pub volume: Option<u8>,
}

/// Minimal MPRIS D-Bus surface. Implemented for real by [`ZbusMprisBus`] and faked in
/// unit tests.
pub trait MprisBus: Send {
    /// Bus names of currently present MPRIS players (`org.mpris.MediaPlayer2.*`).
    fn list_players(&self) -> Result<Vec<String>, HostError>;
    /// The `Identity` property (human-friendly player name).
    fn identity(&self, bus_name: &str) -> Result<String, HostError>;
    /// The `PlaybackStatus` property ("Playing" / "Paused" / "Stopped").
    fn playback_status(&self, bus_name: &str) -> Result<String, HostError>;
    /// `xesam:title` and `xesam:artist` from the `Metadata` property.
    fn metadata(&self, bus_name: &str) -> Result<(String, String), HostError>;
    /// The `Volume` property (0.0-1.0), if exposed.
    fn volume(&self, bus_name: &str) -> Result<Option<f64>, HostError>;
    /// Invoke a Player method (Play/Pause/Next/Previous).
    fn control(&self, bus_name: &str, action: MediaAction) -> Result<(), HostError>;
}

/// High-level MPRIS host service over an [`MprisBus`].
pub struct MprisHost {
    bus: Box<dyn MprisBus>,
}

impl MprisHost {
    pub fn new(bus: Box<dyn MprisBus>) -> Self {
        Self { bus }
    }

    /// Names of present MPRIS players. Empty (not an error) when none are running.
    pub fn players(&self) -> Result<Vec<String>, HostError> {
        self.bus.list_players()
    }

    /// Build a [`MediaStatus`] for one player.
    fn status_for(&self, bus_name: &str) -> Result<MediaStatus, HostError> {
        let identity = self
            .bus
            .identity(bus_name)
            .unwrap_or_else(|_| bus_name.to_string());
        let playing = self
            .bus
            .playback_status(bus_name)
            .map(|s| s == "Playing")
            .unwrap_or(false);
        let (title, artist) = self.bus.metadata(bus_name).unwrap_or_default();
        let volume = self
            .bus
            .volume(bus_name)
            .ok()
            .flatten()
            .map(|v| (v.clamp(0.0, 1.0) * 100.0).round() as u8);
        Ok(MediaStatus {
            playing,
            title,
            artist,
            player: identity,
            volume,
        })
    }

    /// Status of a specific player, or the deterministic first player
    /// ([`first_player`]). Returns a default (empty, not-playing) status when no
    /// player is available — a graceful "no media".
    pub fn status(&self, player: Option<&str>) -> Result<MediaStatus, HostError> {
        if let Some(name) = player {
            return self.status_for(name);
        }
        match first_player(self.bus.list_players()?) {
            Some(first) => self.status_for(&first),
            None => Ok(MediaStatus::default()),
        }
    }

    /// Send a control action to a specific player, or the deterministic first
    /// player ([`first_player`]).
    pub fn control(&self, player: Option<&str>, action: MediaAction) -> Result<(), HostError> {
        let target = match player {
            Some(name) => name.to_string(),
            None => first_player(self.bus.list_players()?)
                .ok_or_else(|| HostError::NotFound("no MPRIS player available".into()))?,
        };
        self.bus.control(&target, action)
    }
}

/// Deterministic "first player" selection (#65). D-Bus `ListNames` has no
/// guaranteed order, and picking whatever comes back first is arbitrary — it can
/// even differ between independent `status()` and `control()` listings, so the
/// app could display one player and send controls to another. Candidates are
/// therefore sorted by bus name (stable, lexicographic) and the smallest wins.
/// Both [`MprisHost::status`] and [`MprisHost::control`] share this rule, so the
/// default target is always the same player for a given bus snapshot.
fn first_player(mut players: Vec<String>) -> Option<String> {
    players.sort();
    players.into_iter().next()
}

/* ------------------------------------------------------------------ */
/* Real zbus-backed MPRIS boundary (feature = "dbus")                  */
/* ------------------------------------------------------------------ */

#[cfg(feature = "dbus")]
pub struct ZbusMprisBus {
    conn: zbus::blocking::Connection,
}

#[cfg(feature = "dbus")]
const MPRIS_OBJECT: &str = "/org/mpris/MediaPlayer2";
#[cfg(feature = "dbus")]
const MPRIS_PREFIX: &str = "org.mpris.MediaPlayer2.";
#[cfg(feature = "dbus")]
const PLAYER_IFACE: &str = "org.mpris.MediaPlayer2.Player";
#[cfg(feature = "dbus")]
const ROOT_IFACE: &str = "org.mpris.MediaPlayer2";

#[cfg(feature = "dbus")]
impl ZbusMprisBus {
    /// Connect to the session bus. Fails with [`HostError::ServiceUnavailable`] when no
    /// session bus is reachable (e.g. no graphical session).
    pub fn session() -> Result<Self, HostError> {
        let conn = zbus::blocking::Connection::session()
            .map_err(|e| HostError::ServiceUnavailable(e.to_string()))?;
        Ok(Self { conn })
    }

    fn get_prop(
        &self,
        bus_name: &str,
        iface: &str,
        prop: &str,
    ) -> Result<zbus::zvariant::OwnedValue, HostError> {
        let proxy = zbus::blocking::Proxy::new(
            &self.conn,
            bus_name.to_string(),
            MPRIS_OBJECT.to_string(),
            "org.freedesktop.DBus.Properties".to_string(),
        )
        .map_err(|e| HostError::Backend(e.to_string()))?;
        let reply = proxy
            .call_method("Get", &(iface, prop))
            .map_err(|e| HostError::Backend(e.to_string()))?;
        let value: zbus::zvariant::OwnedValue = reply
            .body()
            .deserialize()
            .map_err(|e| HostError::Backend(e.to_string()))?;
        Ok(value)
    }
}

#[cfg(feature = "dbus")]
fn value_to_string(v: &zbus::zvariant::Value<'_>) -> Option<String> {
    use zbus::zvariant::Value;
    match v {
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

#[cfg(feature = "dbus")]
impl MprisBus for ZbusMprisBus {
    fn list_players(&self) -> Result<Vec<String>, HostError> {
        let proxy = zbus::blocking::Proxy::new(
            &self.conn,
            "org.freedesktop.DBus".to_string(),
            "/org/freedesktop/DBus".to_string(),
            "org.freedesktop.DBus".to_string(),
        )
        .map_err(|e| HostError::Backend(e.to_string()))?;
        let reply = proxy
            .call_method("ListNames", &())
            .map_err(|e| HostError::ServiceUnavailable(e.to_string()))?;
        let names: Vec<String> = reply
            .body()
            .deserialize()
            .map_err(|e| HostError::Backend(e.to_string()))?;
        Ok(names
            .into_iter()
            .filter(|n| n.starts_with(MPRIS_PREFIX))
            .collect())
    }

    fn identity(&self, bus_name: &str) -> Result<String, HostError> {
        let v = self.get_prop(bus_name, ROOT_IFACE, "Identity")?;
        Ok(value_to_string(&v).unwrap_or_else(|| bus_name.to_string()))
    }

    fn playback_status(&self, bus_name: &str) -> Result<String, HostError> {
        let v = self.get_prop(bus_name, PLAYER_IFACE, "PlaybackStatus")?;
        value_to_string(&v).ok_or_else(|| HostError::Backend("bad PlaybackStatus".into()))
    }

    fn metadata(&self, bus_name: &str) -> Result<(String, String), HostError> {
        use zbus::zvariant::Value;
        let v = self.get_prop(bus_name, PLAYER_IFACE, "Metadata")?;
        let inner: &Value<'_> = &v;
        let zbus::zvariant::Value::Dict(dict) = inner else {
            return Ok((String::new(), String::new()));
        };
        let mut title = String::new();
        let mut artist = String::new();
        for (k, val) in dict.iter() {
            let key = match k {
                Value::Str(s) => s.as_str(),
                _ => continue,
            };
            if key == "xesam:title" {
                if let Some(s) = value_to_string(val) {
                    title = s;
                }
            } else if key == "xesam:artist" {
                // xesam:artist is an array of strings.
                if let Value::Array(arr) = val {
                    let parts: Vec<String> = arr
                        .iter()
                        .filter_map(|item| match item {
                            Value::Str(s) => Some(s.to_string()),
                            _ => None,
                        })
                        .collect();
                    artist = parts.join(", ");
                }
            }
        }
        Ok((title, artist))
    }

    fn volume(&self, bus_name: &str) -> Result<Option<f64>, HostError> {
        use zbus::zvariant::Value;
        let v = self.get_prop(bus_name, PLAYER_IFACE, "Volume")?;
        let inner: &Value<'_> = &v;
        Ok(match inner {
            Value::F64(f) => Some(*f),
            _ => None,
        })
    }

    fn control(&self, bus_name: &str, action: MediaAction) -> Result<(), HostError> {
        let proxy = zbus::blocking::Proxy::new(
            &self.conn,
            bus_name.to_string(),
            MPRIS_OBJECT.to_string(),
            PLAYER_IFACE.to_string(),
        )
        .map_err(|e| HostError::Backend(e.to_string()))?;
        proxy
            .call_method(action.method(), &())
            .map(|_| ())
            .map_err(|e| HostError::Backend(e.to_string()))
    }
}

/* ------------------------------------------------------------------ */
/* Tests against a fake MPRIS bus                                      */
/* ------------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    type ControlLog = Arc<Mutex<Vec<(String, MediaAction)>>>;

    #[derive(Default)]
    struct FakePlayer {
        identity: String,
        status: String,
        title: String,
        artist: String,
        volume: Option<f64>,
    }

    #[derive(Default)]
    struct FakeBus {
        players: HashMap<String, FakePlayer>,
        controls: ControlLog,
        unavailable: bool,
    }

    impl MprisBus for FakeBus {
        fn list_players(&self) -> Result<Vec<String>, HostError> {
            if self.unavailable {
                return Err(HostError::ServiceUnavailable("no session bus".into()));
            }
            let mut names: Vec<String> = self.players.keys().cloned().collect();
            names.sort();
            Ok(names)
        }
        fn identity(&self, bus_name: &str) -> Result<String, HostError> {
            Ok(self
                .players
                .get(bus_name)
                .map(|p| p.identity.clone())
                .unwrap_or_default())
        }
        fn playback_status(&self, bus_name: &str) -> Result<String, HostError> {
            Ok(self
                .players
                .get(bus_name)
                .map(|p| p.status.clone())
                .unwrap_or_else(|| "Stopped".into()))
        }
        fn metadata(&self, bus_name: &str) -> Result<(String, String), HostError> {
            Ok(self
                .players
                .get(bus_name)
                .map(|p| (p.title.clone(), p.artist.clone()))
                .unwrap_or_default())
        }
        fn volume(&self, bus_name: &str) -> Result<Option<f64>, HostError> {
            Ok(self.players.get(bus_name).and_then(|p| p.volume))
        }
        fn control(&self, bus_name: &str, action: MediaAction) -> Result<(), HostError> {
            if !self.players.contains_key(bus_name) {
                return Err(HostError::NotFound(bus_name.into()));
            }
            self.controls
                .lock()
                .expect("controls mutex")
                .push((bus_name.to_string(), action));
            Ok(())
        }
    }

    fn host_with_player() -> (MprisHost, ControlLog) {
        let controls = Arc::new(Mutex::new(Vec::new()));
        let mut players = HashMap::new();
        players.insert(
            "org.mpris.MediaPlayer2.mpv".to_string(),
            FakePlayer {
                identity: "mpv".into(),
                status: "Playing".into(),
                title: "Night Drive".into(),
                artist: "Local Artist".into(),
                volume: Some(0.72),
            },
        );
        let bus = FakeBus {
            players,
            controls: controls.clone(),
            unavailable: false,
        };
        (MprisHost::new(Box::new(bus)), controls)
    }

    #[test]
    fn discovers_players() {
        let (host, _) = host_with_player();
        assert_eq!(host.players().unwrap(), vec!["org.mpris.MediaPlayer2.mpv"]);
    }

    #[test]
    fn maps_state_to_media_status() {
        let (host, _) = host_with_player();
        let st = host.status(None).unwrap();
        assert!(st.playing);
        assert_eq!(st.title, "Night Drive");
        assert_eq!(st.artist, "Local Artist");
        assert_eq!(st.player, "mpv");
        assert_eq!(st.volume, Some(72));
    }

    #[test]
    fn no_players_is_graceful_default() {
        let bus = FakeBus::default();
        let host = MprisHost::new(Box::new(bus));
        let st = host.status(None).unwrap();
        assert!(!st.playing);
        assert_eq!(st.title, "");
        assert_eq!(st.volume, None);
    }

    #[test]
    fn control_targets_first_player() {
        let (host, controls) = host_with_player();
        host.control(None, MediaAction::Pause).unwrap();
        let c = controls.lock().expect("controls mutex");
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].1, MediaAction::Pause);
    }

    #[test]
    fn control_without_players_is_not_found() {
        let bus = FakeBus::default();
        let host = MprisHost::new(Box::new(bus));
        assert!(matches!(
            host.control(None, MediaAction::Play),
            Err(HostError::NotFound(_))
        ));
    }

    /// A bus that deliberately returns players in an unstable, unsorted order
    /// (as a real D-Bus `ListNames` reply may). Pinning deterministic selection
    /// requires a fake that does NOT pre-sort (#65).
    struct UnsortedBus {
        order: Vec<String>,
        players: HashMap<String, FakePlayer>,
        controls: ControlLog,
    }

    impl MprisBus for UnsortedBus {
        fn list_players(&self) -> Result<Vec<String>, HostError> {
            Ok(self.order.clone())
        }
        fn identity(&self, bus_name: &str) -> Result<String, HostError> {
            Ok(self
                .players
                .get(bus_name)
                .map(|p| p.identity.clone())
                .unwrap_or_default())
        }
        fn playback_status(&self, bus_name: &str) -> Result<String, HostError> {
            Ok(self
                .players
                .get(bus_name)
                .map(|p| p.status.clone())
                .unwrap_or_else(|| "Stopped".into()))
        }
        fn metadata(&self, bus_name: &str) -> Result<(String, String), HostError> {
            Ok(self
                .players
                .get(bus_name)
                .map(|p| (p.title.clone(), p.artist.clone()))
                .unwrap_or_default())
        }
        fn volume(&self, bus_name: &str) -> Result<Option<f64>, HostError> {
            Ok(self.players.get(bus_name).and_then(|p| p.volume))
        }
        fn control(&self, bus_name: &str, action: MediaAction) -> Result<(), HostError> {
            self.controls
                .lock()
                .expect("controls mutex")
                .push((bus_name.to_string(), action));
            Ok(())
        }
    }

    fn unsorted_host() -> (MprisHost, ControlLog) {
        let controls = Arc::new(Mutex::new(Vec::new()));
        let mut players = HashMap::new();
        for (name, identity) in [
            ("org.mpris.MediaPlayer2.vlc", "VLC"),
            ("org.mpris.MediaPlayer2.audacious", "Audacious"),
            ("org.mpris.MediaPlayer2.mpv", "mpv"),
        ] {
            players.insert(
                name.to_string(),
                FakePlayer {
                    identity: identity.into(),
                    ..Default::default()
                },
            );
        }
        let bus = UnsortedBus {
            // Deliberately not lexicographic.
            order: vec![
                "org.mpris.MediaPlayer2.vlc".into(),
                "org.mpris.MediaPlayer2.mpv".into(),
                "org.mpris.MediaPlayer2.audacious".into(),
            ],
            players,
            controls: controls.clone(),
        };
        (MprisHost::new(Box::new(bus)), controls)
    }

    #[test]
    fn first_player_selection_is_deterministic_on_an_unsorted_bus() {
        let (host, controls) = unsorted_host();
        // The smallest bus name wins regardless of listing order.
        let st = host.status(None).unwrap();
        assert_eq!(st.player, "Audacious");
        // Control targets the exact same player as status.
        host.control(None, MediaAction::Play).unwrap();
        let c = controls.lock().expect("controls mutex");
        assert_eq!(
            c[0].0,
            "org.mpris.MediaPlayer2.audacious",
            "status and control must agree on the default player"
        );
    }

    #[test]
    fn first_player_of_empty_list_is_none() {
        assert_eq!(first_player(Vec::new()), None);
        assert_eq!(
            first_player(vec!["org.mpris.MediaPlayer2.x".into()]),
            Some("org.mpris.MediaPlayer2.x".into())
        );
    }

    #[test]
    fn unavailable_service_surfaces_structured_error() {
        let bus = FakeBus {
            unavailable: true,
            ..Default::default()
        };
        let host = MprisHost::new(Box::new(bus));
        assert!(matches!(
            host.players(),
            Err(HostError::ServiceUnavailable(_))
        ));
    }
}
