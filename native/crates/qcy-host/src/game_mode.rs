//! Auto Game Mode (issue #13).
//!
//! Turns the earbuds' low-latency game mode on/off in response to a host signal. The
//! design is deliberately event-driven: a [`GameModeSignal`] source pushes
//! [`GameModeEvent`]s, and the pure [`GameModeController`] applies a keyword allowlist
//! and a cooldown so rapid toggling cannot occur. There is no busy polling loop anywhere
//! in this module — the controller only acts when an event arrives.
//!
//! The concrete signal source (which application/player/process is active) is
//! compositor/platform dependent. This module ships the portable, testable core plus a
//! fake source; a real source (e.g. MPRIS `NameOwnerChanged` on the session bus) is
//! documented and can be added behind the `dbus` feature without touching the controller.

use std::time::Duration;

use crate::HostError;

/// An event from a host signal source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameModeEvent {
    /// A candidate (player/app/process) with this name became active.
    Activated(String),
    /// The candidate with this name is no longer active.
    Deactivated(String),
}

/// Keyword allowlist rule. Matching is case-insensitive substring, so a keyword like
/// "game" matches "Steam Big Picture Game Mode".
#[derive(Debug, Clone, Default)]
pub struct GameModeRule {
    pub keywords: Vec<String>,
}

impl GameModeRule {
    pub fn new(keywords: Vec<String>) -> Self {
        Self { keywords }
    }

    /// True when `name` matches any allowlisted keyword.
    pub fn matches(&self, name: &str) -> bool {
        let lower = name.to_lowercase();
        self.keywords
            .iter()
            .any(|k| !k.is_empty() && lower.contains(&k.to_lowercase()))
    }

    /// Desired game-mode state given the currently active candidate, if any.
    pub fn evaluate(&self, active_name: Option<&str>) -> bool {
        match active_name {
            Some(name) => self.matches(name),
            None => false,
        }
    }
}

/// Outcome of feeding one event to the controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameModeDecision {
    /// The desired game-mode state after this event.
    pub game_mode_on: bool,
    /// True when the desired state differs from before the event.
    pub changed: bool,
    /// True when a transition was wanted but suppressed by the cooldown.
    pub suppressed: bool,
}

/// Pure, event-driven Auto Game Mode controller. Deterministic: time is injected as
/// milliseconds so tests do not depend on the wall clock.
pub struct GameModeController {
    rule: GameModeRule,
    cooldown_ms: u64,
    last_transition_ms: Option<u64>,
    /// The currently believed game-mode state.
    on: bool,
    /// The name currently considered active (last Activated that has not deactivated).
    active: Option<String>,
}

impl GameModeController {
    pub fn new(rule: GameModeRule, cooldown: Duration) -> Self {
        Self {
            rule,
            cooldown_ms: cooldown.as_millis() as u64,
            last_transition_ms: None,
            on: false,
            active: None,
        }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    /// Process one event at time `now_ms`. Returns the decision; the caller is
    /// responsible for actually sending the device write (through the central policy).
    pub fn handle(&mut self, event: GameModeEvent, now_ms: u64) -> GameModeDecision {
        match event {
            GameModeEvent::Activated(name) => self.active = Some(name),
            GameModeEvent::Deactivated(_) => self.active = None,
        }
        let desired = self.rule.evaluate(self.active.as_deref());
        if desired == self.on {
            return GameModeDecision {
                game_mode_on: self.on,
                changed: false,
                suppressed: false,
            };
        }
        // A transition is wanted; apply the cooldown.
        let allowed = match self.last_transition_ms {
            Some(last) => now_ms.saturating_sub(last) >= self.cooldown_ms,
            None => true,
        };
        if !allowed {
            return GameModeDecision {
                game_mode_on: self.on,
                changed: false,
                suppressed: true,
            };
        }
        self.on = desired;
        self.last_transition_ms = Some(now_ms);
        GameModeDecision {
            game_mode_on: self.on,
            changed: true,
            suppressed: false,
        }
    }
}

/// An event-driven source of game-mode signals. Implementations must block/wait for the
/// next event rather than poll in a tight loop.
pub trait GameModeSignal {
    /// Return the next event, or `None` when the source is exhausted or unavailable.
    fn next_event(&mut self) -> Option<GameModeEvent>;
}

/* ------------------------------------------------------------------ */
/* MPRIS player presence as the chosen host signal (feature = "dbus")  */
/* ------------------------------------------------------------------ */
/*
 * The concrete Auto Game Mode signal source (issue #13) is MPRIS player presence on
 * the session bus: players appear and disappear as `org.mpris.MediaPlayer2.<name>`
 * bus names, and the bus emits `NameOwnerChanged` for each transition. This is a
 * genuine D-Bus signal subscription — no polling loop, no process scanning.
 *
 * The candidate name fed to the keyword rule is the bus-name suffix after
 * `org.mpris.MediaPlayer2.` (e.g. "vlc", "spotify", "steam"). Game launchers and
 * players that expose MPRIS can therefore be matched by keyword; applications that
 * expose no MPRIS name cannot trigger game mode, which is the safe default.
 */

/// Bus-name prefix of MPRIS players.
pub const MPRIS_NAME_PREFIX: &str = "org.mpris.MediaPlayer2.";

/// Candidate name for keyword matching from an MPRIS bus name: the suffix after the
/// `org.mpris.MediaPlayer2.` prefix. Non-MPRIS names are returned unchanged.
pub fn candidate_name(bus_name: &str) -> &str {
    bus_name.strip_prefix(MPRIS_NAME_PREFIX).unwrap_or(bus_name)
}

/// Pure presence diff: given the previously known player set and a fresh snapshot,
/// emit one [`GameModeEvent`] per disappeared and per appeared candidate. Used for
/// initial scans and reconciliation; deterministic and unit-tested.
pub fn presence_events(
    previous: &std::collections::BTreeSet<String>,
    current: &std::collections::BTreeSet<String>,
) -> Vec<GameModeEvent> {
    let mut events = Vec::new();
    for gone in previous.difference(current) {
        events.push(GameModeEvent::Deactivated(gone.clone()));
    }
    for appeared in current.difference(previous) {
        events.push(GameModeEvent::Activated(appeared.clone()));
    }
    events
}

/// MPRIS player-presence signal over the session bus. Blocks in [`next_event`] on the
/// `NameOwnerChanged` signal stream — event-driven by construction.
#[cfg(feature = "dbus")]
pub struct MprisPresenceSignal {
    conn: zbus::blocking::Connection,
    iter: zbus::blocking::MessageIterator,
    pending: std::collections::VecDeque<GameModeEvent>,
}

#[cfg(feature = "dbus")]
impl MprisPresenceSignal {
    /// Connect to the session bus and subscribe to MPRIS name changes. Fails with
    /// [`HostError::ServiceUnavailable`] when no session bus is reachable.
    pub fn session() -> Result<Self, HostError> {
        let conn = zbus::blocking::Connection::session()
            .map_err(|e| HostError::ServiceUnavailable(e.to_string()))?;
        let rule_err = |e: zbus::Error| HostError::Backend(e.to_string());
        let rule = zbus::MatchRule::builder()
            .msg_type(zbus::message::Type::Signal)
            .sender("org.freedesktop.DBus")
            .map_err(rule_err)?
            .interface("org.freedesktop.DBus")
            .map_err(rule_err)?
            .member("NameOwnerChanged")
            .map_err(rule_err)?
            .arg0ns("org.mpris.MediaPlayer2")
            .map_err(rule_err)?
            .build();
        let iter = zbus::blocking::MessageIterator::for_match_rule(rule, &conn, Some(16))
            .map_err(|e| HostError::Backend(e.to_string()))?;
        let mut signal = Self {
            conn,
            iter,
            pending: std::collections::VecDeque::new(),
        };
        signal.scan_initial_players();
        Ok(signal)
    }

    /// Buffer [`GameModeEvent::Activated`] for players already present at subscription
    /// time. A failure here is non-fatal: the signal still delivers live changes.
    fn scan_initial_players(&mut self) {
        let proxy = match zbus::blocking::fdo::DBusProxy::new(&self.conn) {
            Ok(p) => p,
            Err(_) => return,
        };
        let names = match proxy.list_names() {
            Ok(n) => n,
            Err(_) => return,
        };
        for name in names {
            if let Some(candidate) = name.as_str().strip_prefix(MPRIS_NAME_PREFIX) {
                self.pending
                    .push_back(GameModeEvent::Activated(candidate.to_string()));
            }
        }
    }
}

#[cfg(feature = "dbus")]
impl GameModeSignal for MprisPresenceSignal {
    fn next_event(&mut self) -> Option<GameModeEvent> {
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        // Block until the bus delivers the next NameOwnerChanged signal.
        for msg in &mut self.iter {
            let Ok(msg) = msg else { continue };
            let Ok(body) = msg.body().deserialize::<(String, String, String)>() else {
                continue;
            };
            let (name, old_owner, new_owner) = body;
            let Some(candidate) = name.strip_prefix(MPRIS_NAME_PREFIX) else {
                continue;
            };
            let candidate = candidate.to_string();
            if !new_owner.is_empty() {
                return Some(GameModeEvent::Activated(candidate));
            }
            if !old_owner.is_empty() {
                return Some(GameModeEvent::Deactivated(candidate));
            }
        }
        None
    }
}

/// A fake, scriptable signal source for tests and mock mode.
#[derive(Default)]
pub struct ScriptedSignal {
    events: std::collections::VecDeque<GameModeEvent>,
}

impl ScriptedSignal {
    pub fn new(events: Vec<GameModeEvent>) -> Self {
        Self {
            events: events.into(),
        }
    }
}

impl GameModeSignal for ScriptedSignal {
    fn next_event(&mut self) -> Option<GameModeEvent> {
        self.events.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(keywords: &[&str], cooldown_ms: u64) -> GameModeController {
        GameModeController::new(
            GameModeRule::new(keywords.iter().map(|s| s.to_string()).collect()),
            Duration::from_millis(cooldown_ms),
        )
    }

    #[test]
    fn keyword_match_is_case_insensitive_substring() {
        let rule = GameModeRule::new(vec!["game".into()]);
        assert!(rule.matches("Steam Game Mode"));
        assert!(rule.matches("GAME"));
        assert!(!rule.matches("music player"));
    }

    #[test]
    fn activation_matching_keyword_turns_game_mode_on() {
        let mut c = controller(&["game"], 1000);
        let d = c.handle(GameModeEvent::Activated("My Game".into()), 0);
        assert!(d.changed);
        assert!(d.game_mode_on);
        assert!(c.is_on());
    }

    #[test]
    fn deactivation_turns_game_mode_off() {
        let mut c = controller(&["game"], 1000);
        c.handle(GameModeEvent::Activated("My Game".into()), 0);
        let d = c.handle(GameModeEvent::Deactivated("My Game".into()), 2000);
        assert!(d.changed);
        assert!(!d.game_mode_on);
    }

    #[test]
    fn non_matching_activation_does_nothing() {
        let mut c = controller(&["game"], 1000);
        let d = c.handle(GameModeEvent::Activated("music player".into()), 0);
        assert!(!d.changed);
        assert!(!d.game_mode_on);
    }

    #[test]
    fn cooldown_suppresses_rapid_toggling() {
        let mut c = controller(&["game"], 1000);
        let on = c.handle(GameModeEvent::Activated("game".into()), 0);
        assert!(on.changed);
        // Immediately deactivate then reactivate within the cooldown window.
        let off = c.handle(GameModeEvent::Deactivated("game".into()), 100);
        assert!(off.suppressed);
        assert!(!off.changed);
        assert!(c.is_on()); // still on because the off transition was suppressed
                            // After the cooldown elapses, the transition is allowed.
        let off2 = c.handle(GameModeEvent::Deactivated("game".into()), 1200);
        assert!(off2.changed);
        assert!(!off2.game_mode_on);
    }

    #[test]
    fn scripted_signal_drives_the_controller_event_by_event() {
        let mut sig = ScriptedSignal::new(vec![
            GameModeEvent::Activated("game".into()),
            GameModeEvent::Deactivated("game".into()),
        ]);
        let mut c = controller(&["game"], 0);
        let mut t = 0u64;
        let mut states = Vec::new();
        while let Some(ev) = sig.next_event() {
            let d = c.handle(ev, t);
            states.push(d.game_mode_on);
            t += 10;
        }
        assert_eq!(states, vec![true, false]);
    }

    #[test]
    fn empty_keyword_allowlist_never_activates() {
        let mut c = controller(&[], 1000);
        let d = c.handle(GameModeEvent::Activated("anything".into()), 0);
        assert!(!d.changed);
        assert!(!d.game_mode_on);
    }

    #[test]
    fn candidate_name_strips_the_mpris_prefix() {
        assert_eq!(candidate_name("org.mpris.MediaPlayer2.vlc"), "vlc");
        assert_eq!(
            candidate_name("org.mpris.MediaPlayer2.plasma-browser-integration"),
            "plasma-browser-integration"
        );
        // Non-MPRIS names pass through unchanged.
        assert_eq!(candidate_name("some.other.name"), "some.other.name");
    }

    #[test]
    fn presence_diff_emits_deactivated_then_activated() {
        use std::collections::BTreeSet;
        let prev: BTreeSet<String> = ["vlc", "spotify"].iter().map(|s| s.to_string()).collect();
        let curr: BTreeSet<String> = ["spotify", "steam"].iter().map(|s| s.to_string()).collect();
        let events = presence_events(&prev, &curr);
        assert_eq!(
            events,
            vec![
                GameModeEvent::Deactivated("vlc".into()),
                GameModeEvent::Activated("steam".into()),
            ]
        );
        // Identical sets produce no events.
        assert!(presence_events(&curr, &curr).is_empty());
    }

    #[test]
    fn full_pipeline_signal_to_controller_to_device_sink() {
        // End-to-end (without hardware): an MPRIS-style presence signal drives the
        // controller, and only the controller's *changed* decisions reach the device
        // write sink. In the real application that sink is the central write policy +
        // transport; here it is a fake that records requested game-mode states.
        let mut signal = ScriptedSignal::new(vec![
            GameModeEvent::Activated("steam".into()),
            GameModeEvent::Activated("unrelated-player".into()),
            GameModeEvent::Deactivated("steam".into()),
        ]);
        let mut controller = controller(&["steam", "game"], 500);
        let mut device_writes: Vec<bool> = Vec::new();
        let mut now_ms = 0u64;
        while let Some(event) = signal.next_event() {
            let decision = controller.handle(event, now_ms);
            if decision.changed {
                device_writes.push(decision.game_mode_on);
            }
            now_ms += 1000;
        }
        assert_eq!(device_writes, vec![true, false]);
    }
}
