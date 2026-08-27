//! BlueZ D-Bus transport backend (issue #7).
//!
//! Talks to the system BlueZ stack through its D-Bus GATT API — it never shells out to
//! interactive tools, never requires root, and never reconfigures the daemon. The D-Bus
//! access is isolated behind the [`BlueZBus`] trait so the object-mapping, discovery,
//! characteristic-resolution and policy logic can all be unit-tested against a fake bus
//! when no Bluetooth daemon is available.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::policy::{WritePolicy, SERVICE_MAIN};
use crate::{DiscoveredDevice, Transport, TransportError};

/* ------------------------------------------------------------------ */
/* Object-path mapping (pure, deterministic, unit-tested)              */
/* ------------------------------------------------------------------ */

/// Normalize a MAC address to the BlueZ object-path fragment (`AA:BB..` -> `AA_BB..`).
pub fn normalize_mac(mac: &str) -> String {
    mac.trim().to_ascii_uppercase().replace([':', '-'], "_")
}

/// BlueZ object path for a device on an adapter, e.g. `/org/bluez/hci0/dev_AA_BB_..`.
pub fn device_path(adapter: &str, mac: &str) -> String {
    format!("/org/bluez/{adapter}/dev_{}", normalize_mac(mac))
}

/* ------------------------------------------------------------------ */
/* Fakeable D-Bus boundary                                             */
/* ------------------------------------------------------------------ */

/// A property value read from a BlueZ managed object.
#[derive(Debug, Clone, PartialEq)]
pub enum PropValue {
    Str(String),
    I16(i16),
    Bool(bool),
    Bytes(Vec<u8>),
    /// String arrays, e.g. the device `UUIDs` property (service UUIDs advertised
    /// or resolved for the device).
    StrArray(Vec<String>),
}

/// A BlueZ object: its path plus per-interface property maps.
#[derive(Debug, Clone, Default)]
pub struct BlueZObject {
    pub path: String,
    pub interfaces: HashMap<String, HashMap<String, PropValue>>,
}

/// Minimal D-Bus surface the BlueZ backend needs. Implemented for real by
/// [`ZbusBlueZBus`] and faked in unit tests.
pub trait BlueZBus: Send {
    fn managed_objects(&self) -> Result<Vec<BlueZObject>, TransportError>;
    fn start_discovery(&self, adapter: &str) -> Result<(), TransportError>;
    /// Stop discovery. Implementations must tolerate "not discovering" errors.
    fn stop_discovery(&self, adapter: &str) -> Result<(), TransportError>;
    fn device_connect(&self, device_path: &str) -> Result<(), TransportError>;
    fn device_disconnect(&self, device_path: &str) -> Result<(), TransportError>;
    fn read_value(&self, char_path: &str) -> Result<Vec<u8>, TransportError>;
    fn write_value(&self, char_path: &str, bytes: &[u8]) -> Result<(), TransportError>;
    fn start_notify(&self, char_path: &str) -> Result<(), TransportError>;
    fn stop_notify(&self, char_path: &str) -> Result<(), TransportError>;
    /// Object path of the device holding an active (or being-acquired)
    /// HFP/HSP SCO media transport, if any. Live HT08 evidence (#52): while
    /// SCO is held, LE connection initiation is aborted by the host on some
    /// controllers, so the transport must surface this before trying LE.
    fn active_hfp_transport(&self) -> Result<Option<String>, TransportError>;
}

/* ------------------------------------------------------------------ */
/* BlueZ transport                                                     */
/* ------------------------------------------------------------------ */

const DEVICE_IFACE: &str = "org.bluez.Device1";
const SERVICE_IFACE: &str = "org.bluez.GattService1";
const CHAR_IFACE: &str = "org.bluez.GattCharacteristic1";
const TRANSPORT_IFACE: &str = "org.bluez.MediaTransport1";

/// UUID prefixes (lowercase, 16-bit form inside the full 128-bit UUID) for
/// profiles that carry SCO: HSP headset/gateway and HFP handsfree/gateway.
/// A2DP source/sink are intentionally excluded — music playback does not
/// block LE.
const SCO_UUID_PREFIXES: &[&str] = &["00001108", "00001109", "0000111e", "0000111f"];

/// Scan managed objects for a MediaTransport1 in `active`/`pending` state
/// whose UUID is an HFP/HSP (SCO) profile. Returns the owning device path.
fn find_active_hfp_transport(objects: &[BlueZObject]) -> Option<String> {
    for obj in objects {
        let Some(props) = obj.interfaces.get(TRANSPORT_IFACE) else {
            continue;
        };
        let uuid = match props.get("UUID") {
            Some(PropValue::Str(u)) => u.to_ascii_lowercase(),
            _ => continue,
        };
        if !SCO_UUID_PREFIXES.iter().any(|pfx| uuid.starts_with(pfx)) {
            continue;
        }
        let state = match props.get("State") {
            Some(PropValue::Str(s)) => s.as_str(),
            _ => "idle",
        };
        if state != "active" && state != "pending" {
            continue;
        }
        if let Some(PropValue::Str(device)) = props.get("Device") {
            return Some(device.clone());
        }
        return Some(obj.path.clone());
    }
    None
}

/// Name prefixes treated as candidate QCY devices during discovery.
const QCY_NAME_PREFIXES: &[&str] = &["QCY", "MeloBuds"];
/// Name fragments that prove the HT08 model (anything else stays unknown/read-only).
const HT08_NAME_FRAGMENTS: &[&str] = &["MeloBuds Pro", "HT08"];

pub struct BlueZTransport {
    bus: Box<dyn BlueZBus>,
    policy: WritePolicy,
    experimental_opt_in: bool,
    adapter: String,
    device_path: Option<String>,
    /// Resolved characteristic UUID (lowercase) -> object path.
    chars: HashMap<String, String>,
    /// Model evidence for the connected device, resolved at connect time from the
    /// device's advertised name. Unknown models stay read-only: writes are denied
    /// even though the configured policy is the HT08 one.
    connected_model_known: bool,
    /// Bounded discovery windows (injectable so tests run without real waits).
    pub(crate) scan_window: Duration,
    pub(crate) le_fallback_window: Duration,
}

/// How long `scan` watches for advertising devices before returning.
const SCAN_WINDOW: Duration = Duration::from_secs(6);
/// After the first candidate appears, wait this long for more before returning.
const SCAN_GRACE: Duration = Duration::from_millis(1500);
/// How long the dual-mode connect fallback searches for a BLE identity.
const LE_FALLBACK_WINDOW: Duration = Duration::from_secs(10);
/// Discovery poll interval. Bounded polling, not an open-ended loop: discovery
/// results arrive asynchronously in BlueZ and the blocking D-Bus API has no
/// signal-wait surface here.
const DISCOVERY_POLL: Duration = Duration::from_millis(400);

impl BlueZTransport {
    pub fn new(bus: Box<dyn BlueZBus>, policy: WritePolicy) -> Self {
        Self {
            bus,
            policy,
            experimental_opt_in: false,
            adapter: "hci0".to_string(),
            device_path: None,
            chars: HashMap::new(),
            connected_model_known: false,
            scan_window: SCAN_WINDOW,
            le_fallback_window: LE_FALLBACK_WINDOW,
        }
    }

    pub fn with_adapter(mut self, adapter: impl Into<String>) -> Self {
        self.adapter = adapter.into();
        self
    }

    fn prop_str<'a>(obj: &'a BlueZObject, iface: &str, key: &str) -> Option<&'a str> {
        match obj.interfaces.get(iface)?.get(key)? {
            PropValue::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    fn is_qcy_name(name: &str) -> bool {
        QCY_NAME_PREFIXES.iter().any(|p| name.starts_with(p))
    }

    fn is_known_model(name: &str) -> bool {
        HT08_NAME_FRAGMENTS.iter().any(|f| name.contains(f))
    }

    /// The device's display name (BlueZ `Alias`, falling back to `Name`).
    fn device_name_of(obj: &BlueZObject) -> Option<&str> {
        Self::prop_str(obj, DEVICE_IFACE, "Alias")
            .or_else(|| Self::prop_str(obj, DEVICE_IFACE, "Name"))
    }

    /// A device object as a scan candidate: QCY-ish name, address and RSSI.
    fn candidate_device(obj: &BlueZObject) -> Option<DiscoveredDevice> {
        if !obj.interfaces.contains_key(DEVICE_IFACE) {
            return None;
        }
        let name = Self::device_name_of(obj)?;
        if !Self::is_qcy_name(name) {
            return None;
        }
        let address = Self::prop_str(obj, DEVICE_IFACE, "Address")?;
        let rssi = match obj.interfaces.get(DEVICE_IFACE).and_then(|p| p.get("RSSI")) {
            Some(PropValue::I16(v)) => Some(*v),
            _ => None,
        };
        Some(DiscoveredDevice {
            address: address.to_string(),
            name: name.to_string(),
            rssi,
            model_known: Self::is_known_model(name),
        })
    }

    /// True when the device object lists the vendor main service in its `UUIDs`
    /// property (advertisement service list or resolved GATT) — strong evidence
    /// that this identity speaks the QCY vendor protocol.
    fn advertises_main_service(obj: &BlueZObject) -> bool {
        match obj
            .interfaces
            .get(DEVICE_IFACE)
            .and_then(|p| p.get("UUIDs"))
        {
            Some(PropValue::StrArray(uuids)) => {
                uuids.iter().any(|u| u.eq_ignore_ascii_case(SERVICE_MAIN))
            }
            _ => false,
        }
    }

    /// Resolve and cache the characteristics of the main service for the connected device.
    fn resolve_chars(&mut self) -> Result<(), TransportError> {
        let device_path = self
            .device_path
            .clone()
            .ok_or(TransportError::Disconnected)?;
        let objects = self.bus.managed_objects()?;

        let service_prefix = format!("{device_path}/");
        let mut service_path: Option<String> = None;
        for obj in &objects {
            if !obj.path.starts_with(&service_prefix) {
                continue;
            }
            if let Some(uuid) = Self::prop_str(obj, SERVICE_IFACE, "UUID") {
                if uuid.eq_ignore_ascii_case(SERVICE_MAIN) {
                    service_path = Some(obj.path.clone());
                }
            }
        }
        let service_path =
            service_path.ok_or_else(|| TransportError::NotFound("main GATT service".into()))?;

        self.chars.clear();
        let char_prefix = format!("{service_path}/");
        for obj in &objects {
            if !obj.path.starts_with(&char_prefix) {
                continue;
            }
            if let Some(uuid) = Self::prop_str(obj, CHAR_IFACE, "UUID") {
                self.chars
                    .insert(uuid.to_ascii_lowercase(), obj.path.clone());
            }
        }
        Ok(())
    }

    /// Model evidence for a connected device path: the device object's Alias/Name
    /// must prove the model (HT08 name fragments). Anything else stays unknown and
    /// therefore read-only. Absent evidence never upgrades to known.
    fn device_model_known(&self, path: &str) -> bool {
        let objects = match self.bus.managed_objects() {
            Ok(objects) => objects,
            Err(_) => return false,
        };
        objects
            .iter()
            .find(|obj| obj.path == path)
            .and_then(|obj| {
                Self::prop_str(obj, DEVICE_IFACE, "Alias")
                    .or_else(|| Self::prop_str(obj, DEVICE_IFACE, "Name"))
            })
            .is_some_and(Self::is_known_model)
    }

    fn char_path(&self, char_uuid: &str) -> Result<String, TransportError> {
        self.chars
            .get(&char_uuid.to_ascii_lowercase())
            .cloned()
            .ok_or_else(|| TransportError::NotFound(char_uuid.to_string()))
    }

    /// Forget the current session: device path, model attestation and every
    /// cached characteristic handle. The bus disconnect is best-effort — the
    /// session identity must be invalidated even if the bus call fails.
    fn invalidate_session(&mut self) {
        let path = self.device_path.take();
        self.chars.clear();
        self.connected_model_known = false;
        if let Some(path) = path {
            let _ = self.bus.device_disconnect(&path);
        }
    }

    /// True when the device object already reports `Connected = true` (any link —
    /// e.g. the earbuds are connected for BR/EDR audio). A redundant
    /// `Device1.Connect()` on such an object fails with `br-connection-busy`,
    /// so the connect step is skipped and resolution proceeds directly.
    fn device_already_connected(&self, path: &str) -> bool {
        self.bus
            .managed_objects()
            .ok()
            .map(|objects| {
                objects.iter().any(|o| {
                    o.path == path
                        && matches!(
                            o.interfaces
                                .get(DEVICE_IFACE)
                                .and_then(|p| p.get("Connected")),
                            Some(PropValue::Bool(true))
                        )
                })
            })
            .unwrap_or(false)
    }

    /// Connect one specific device object and resolve the vendor characteristics.
    /// Leaves the transport fully disconnected on any failure (no half state).
    fn try_connect_path(&mut self, path: &str) -> Result<(), TransportError> {
        if !self.device_already_connected(path) {
            match self.bus.device_connect(path) {
                Ok(()) => {}
                // The link is already up or BlueZ is already handling it (a
                // concurrent/previous connect answers `br-connection-busy` or
                // `AlreadyConnected`). Proceed to characteristic resolution;
                // resolution and the dual-mode fallback decide the outcome.
                Err(TransportError::Bus(msg))
                    if msg.contains("br-connection-busy") || msg.contains("AlreadyConnected") => {}
                Err(e) => return Err(e),
            }
        }
        self.device_path = Some(path.to_string());
        self.connected_model_known = self.device_model_known(path);
        if let Err(e) = self.resolve_chars() {
            self.device_path = None;
            self.chars.clear();
            self.connected_model_known = false;
            return Err(e);
        }
        Ok(())
    }

    /// Bounded discovery for a BLE identity carrying the vendor GATT service.
    /// Candidates: same display name as the originally requested device, or the
    /// vendor main service in their `UUIDs`. Strongest RSSI tried first. On
    /// success the transport is connected through the found identity.
    fn find_le_identity(&mut self, original_address: &str, original_name: Option<&str>) -> bool {
        if self.bus.start_discovery(&self.adapter).is_err() {
            return false;
        }
        let deadline = Instant::now() + self.le_fallback_window;
        let mut tried: std::collections::HashSet<String> =
            [original_address.to_string()].into_iter().collect();
        let mut connected = false;
        loop {
            let mut candidates: Vec<(String, Option<i16>)> = Vec::new();
            if let Ok(objects) = self.bus.managed_objects() {
                for obj in &objects {
                    if !obj.interfaces.contains_key(DEVICE_IFACE) {
                        continue;
                    }
                    let Some(address) = Self::prop_str(obj, DEVICE_IFACE, "Address") else {
                        continue;
                    };
                    if tried.contains(address) {
                        continue;
                    }
                    let name_matches = match (original_name, Self::device_name_of(obj)) {
                        (Some(orig), Some(name)) => {
                            !orig.is_empty() && name.eq_ignore_ascii_case(orig)
                        }
                        _ => false,
                    };
                    if !name_matches && !Self::advertises_main_service(obj) {
                        continue;
                    }
                    let rssi = match obj.interfaces.get(DEVICE_IFACE).and_then(|p| p.get("RSSI")) {
                        Some(PropValue::I16(v)) => Some(*v),
                        _ => None,
                    };
                    candidates.push((address.to_string(), rssi));
                }
            }
            // Strongest signal first (None sorts last).
            candidates.sort_by_key(|c| std::cmp::Reverse(c.1));
            for (address, _) in candidates {
                tried.insert(address.clone());
                let path = device_path(&self.adapter, &address);
                if self.try_connect_path(&path).is_ok() {
                    connected = true;
                    break;
                }
            }
            if connected || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(DISCOVERY_POLL);
        }
        let _ = self.bus.stop_discovery(&self.adapter);
        connected
    }
}

impl Transport for BlueZTransport {
    fn scan(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        self.bus.start_discovery(&self.adapter)?;
        // Bounded discovery: BlueZ reports devices asynchronously, so watch the
        // object tree for a window instead of trusting a single instant snapshot.
        let deadline = Instant::now() + self.scan_window;
        let mut best: HashMap<String, DiscoveredDevice> = HashMap::new();
        let mut first_seen: Option<Instant> = None;
        loop {
            for obj in self.bus.managed_objects()? {
                if let Some(dev) = Self::candidate_device(&obj) {
                    best.insert(dev.address.clone(), dev);
                }
            }
            if !best.is_empty() && first_seen.is_none() {
                first_seen = Some(Instant::now());
            }
            let grace_done = first_seen.is_some_and(|t| t.elapsed() >= SCAN_GRACE);
            if grace_done || Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(DISCOVERY_POLL);
        }
        self.bus.stop_discovery(&self.adapter)?;
        let mut out: Vec<DiscoveredDevice> = best.into_values().collect();
        out.sort_by(|a, b| a.address.cmp(&b.address));
        Ok(out)
    }

    fn connected_devices(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        // No discovery needed: devices the host is already connected to are
        // present in the BlueZ object tree with `Connected = true` (they may
        // not be advertising, so a scan window can miss them).
        let mut out: Vec<DiscoveredDevice> = Vec::new();
        for obj in self.bus.managed_objects()? {
            if !matches!(
                obj.interfaces
                    .get(DEVICE_IFACE)
                    .and_then(|p| p.get("Connected")),
                Some(PropValue::Bool(true))
            ) {
                continue;
            }
            if let Some(dev) = Self::candidate_device(&obj) {
                out.push(dev);
                continue;
            }
            // Renamed device: no QCY-ish name, but the vendor main service in
            // the object's `UUIDs` is strong evidence it speaks the QCY vendor
            // protocol. The model stays unproven (read-only until confirmed).
            if Self::advertises_main_service(&obj) {
                let Some(address) = Self::prop_str(&obj, DEVICE_IFACE, "Address") else {
                    continue;
                };
                out.push(DiscoveredDevice {
                    address: address.to_string(),
                    name: Self::device_name_of(&obj)
                        .unwrap_or("QCY device")
                        .to_string(),
                    rssi: None,
                    model_known: false,
                });
            }
        }
        out.sort_by(|a, b| a.address.cmp(&b.address));
        Ok(out)
    }

    fn connect(&mut self, address: &str) -> Result<(), TransportError> {
        // Transactional replacement: a new connection attempt must never
        // inherit characteristic handles from a previous device/session, and
        // any failure below must leave the transport fully disconnected.
        self.invalidate_session();
        let path = device_path(&self.adapter, address);
        let primary = self.try_connect_path(&path);
        if primary.is_ok() {
            return Ok(());
        }
        // Dual-mode fallback: earbuds often pair as a BR/EDR audio device while
        // the QCY vendor protocol lives on a separate BLE/GATT identity that has
        // no GATT under this object. Search for that identity in a bounded
        // discovery: a device with the same name as the requested one, or one
        // advertising the vendor main service.
        let original_name = self.bus.managed_objects().ok().and_then(|objects| {
            objects
                .iter()
                .find(|o| o.path == path)
                .and_then(|o| Self::device_name_of(o).map(|s| s.to_string()))
        });
        // HFP preflight (live HT08 evidence, #52): while an HFP/SCO session is
        // held, this controller aborts LE connection initiation before any HCI
        // command is issued (le-connection-abort-by-local). Fail fast with an
        // actionable diagnostic instead of burning the discovery window.
        if let Some(holder) = self.bus.active_hfp_transport()? {
            return Err(TransportError::HfpBlocked(format!(" ({holder})")));
        }
        if self.find_le_identity(address, original_name.as_deref()) {
            return Ok(());
        }
        match primary {
            Err(TransportError::NotFound(what)) => Err(TransportError::NotFound(format!(
                "{what} for {address}; if these earbuds are paired for audio only, their                  BLE identity may be asleep — open the charging case or disconnect the                  audio, then scan and connect again"
            ))),
            other => other,
        }
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        if let Some(path) = self.device_path.take() {
            self.chars.clear();
            self.connected_model_known = false;
            self.bus.device_disconnect(&path)?;
        }
        Ok(())
    }

    fn is_connected(&mut self) -> Result<bool, TransportError> {
        let Some(path) = self.device_path.clone() else {
            return Ok(false);
        };
        // Link loss appears as the device object's Connected property flipping
        // to false (or the object vanishing); both mean the session is gone.
        let objects = self.bus.managed_objects()?;
        Ok(objects.iter().any(|o| {
            o.path == path
                && matches!(
                    o.interfaces
                        .get(DEVICE_IFACE)
                        .and_then(|p| p.get("Connected")),
                    Some(PropValue::Bool(true))
                )
        }))
    }

    fn read(&mut self, char_uuid: &str) -> Result<Vec<u8>, TransportError> {
        if self.device_path.is_none() {
            return Err(TransportError::Disconnected);
        }
        let path = self.char_path(char_uuid)?;
        self.bus.read_value(&path)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.device_path.is_none() {
            return Err(TransportError::Disconnected);
        }
        if !self.connected_model_known {
            return Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice,
            ));
        }
        self.policy
            .authorize_frame(bytes, self.experimental_opt_in)
            .map_err(TransportError::Denied)?;
        let path = self.char_path(crate::policy::CHAR_COMMAND_WRITE)?;
        self.bus.write_value(&path, bytes)
    }

    fn write_direct(&mut self, char_uuid: &str, bytes: &[u8]) -> Result<(), TransportError> {
        if self.device_path.is_none() {
            return Err(TransportError::Disconnected);
        }
        if !self.connected_model_known {
            return Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice,
            ));
        }
        self.policy
            .authorize_direct(char_uuid, bytes, self.experimental_opt_in)
            .map_err(TransportError::Denied)?;
        let path = self.char_path(char_uuid)?;
        self.bus.write_value(&path, bytes)
    }

    fn subscribe(&mut self, char_uuid: &str) -> Result<(), TransportError> {
        if self.device_path.is_none() {
            return Err(TransportError::Disconnected);
        }
        let path = self.char_path(char_uuid)?;
        self.bus.start_notify(&path)
    }

    fn set_experimental_opt_in(&mut self, on: bool) {
        self.experimental_opt_in = on;
    }

    fn attest_model_known(&mut self) {
        if self.device_path.is_some() {
            self.connected_model_known = true;
        }
    }
}

/* ------------------------------------------------------------------ */
/* Real zbus-backed D-Bus boundary (feature = "bluez")                 */
/* ------------------------------------------------------------------ */

#[cfg(feature = "bluez")]
pub struct ZbusBlueZBus {
    conn: zbus::blocking::Connection,
}

#[cfg(feature = "bluez")]
impl ZbusBlueZBus {
    /// Connect to the system bus. Fails with [`TransportError::AdapterOff`] when no
    /// system bus is reachable.
    pub fn system() -> Result<Self, TransportError> {
        let conn = zbus::blocking::Connection::system().map_err(|_| TransportError::AdapterOff)?;
        Ok(Self { conn })
    }

    fn proxy(&self, path: &str, iface: &str) -> Result<zbus::blocking::Proxy<'_>, TransportError> {
        zbus::blocking::Proxy::new(&self.conn, "org.bluez", path.to_string(), iface.to_string())
            .map_err(|e| TransportError::Bus(e.to_string()))
    }

    fn map_dbus_err(e: zbus::Error) -> TransportError {
        let msg = e.to_string();
        if msg.contains("NotPermitted") || msg.contains("NotAuthorized") {
            TransportError::PermissionDenied
        } else if msg.contains("NoReply") || msg.contains("TimedOut") || msg.contains("InProgress")
        {
            TransportError::Timeout
        } else if msg.contains("NoSuchAdapter") {
            TransportError::AdapterOff
        } else if msg.contains("page-timeout")
            || msg.contains("connect-failed")
            || msg.contains("le-connection-abort")
        {
            TransportError::DeviceOutOfRange
        } else if msg.contains("NotReady") || msg.contains("NotConnected") {
            TransportError::Disconnected
        } else {
            TransportError::Bus(msg)
        }
    }

    fn to_prop(ov: &zbus::zvariant::OwnedValue) -> Option<PropValue> {
        use zbus::zvariant::Value;
        let v: &Value<'_> = ov;
        match v {
            Value::Str(s) => Some(PropValue::Str(s.to_string())),
            Value::I16(i) => Some(PropValue::I16(*i)),
            Value::Bool(b) => Some(PropValue::Bool(*b)),
            Value::Array(a) => {
                let bytes: Result<Vec<u8>, ()> = a
                    .iter()
                    .map(|item| match item {
                        Value::U8(b) => Ok(*b),
                        _ => Err(()),
                    })
                    .collect();
                if let Ok(bytes) = bytes {
                    return Some(PropValue::Bytes(bytes));
                }
                let strings: Result<Vec<String>, ()> = a
                    .iter()
                    .map(|item| match item {
                        Value::Str(s) => Ok(s.to_string()),
                        _ => Err(()),
                    })
                    .collect();
                strings.ok().map(PropValue::StrArray)
            }
            _ => None,
        }
    }
}

#[cfg(feature = "bluez")]
impl BlueZBus for ZbusBlueZBus {
    fn managed_objects(&self) -> Result<Vec<BlueZObject>, TransportError> {
        let om = zbus::blocking::fdo::ObjectManagerProxy::builder(&self.conn)
            .destination("org.bluez")
            .map_err(|e| TransportError::Bus(e.to_string()))?
            .path("/")
            .map_err(|e| TransportError::Bus(e.to_string()))?
            .build()
            .map_err(|e| TransportError::Bus(e.to_string()))?;
        let managed = om
            .get_managed_objects()
            .map_err(|e| Self::map_dbus_err(e.into()))?;
        let mut out = Vec::new();
        for (path, ifaces) in managed {
            let mut obj = BlueZObject {
                path: path.to_string(),
                interfaces: HashMap::new(),
            };
            for (iface, props) in ifaces {
                let mut map = HashMap::new();
                for (key, value) in props {
                    if let Some(pv) = Self::to_prop(&value) {
                        map.insert(key, pv);
                    }
                }
                obj.interfaces.insert(iface.to_string(), map);
            }
            out.push(obj);
        }
        Ok(out)
    }

    fn start_discovery(&self, adapter: &str) -> Result<(), TransportError> {
        let proxy = self.proxy(&format!("/org/bluez/{adapter}"), "org.bluez.Adapter1")?;
        proxy
            .call_method("StartDiscovery", &())
            .map(|_| ())
            .map_err(Self::map_dbus_err)
    }

    fn stop_discovery(&self, adapter: &str) -> Result<(), TransportError> {
        let proxy = self.proxy(&format!("/org/bluez/{adapter}"), "org.bluez.Adapter1")?;
        // Best-effort cleanup: tolerate "not discovering" and vanishing adapters.
        let _ = proxy.call_method("StopDiscovery", &());
        Ok(())
    }

    fn device_connect(&self, device_path: &str) -> Result<(), TransportError> {
        let proxy = self.proxy(device_path, "org.bluez.Device1")?;
        proxy
            .call_method("Connect", &())
            .map(|_| ())
            .map_err(Self::map_dbus_err)
    }

    fn device_disconnect(&self, device_path: &str) -> Result<(), TransportError> {
        let proxy = self.proxy(device_path, "org.bluez.Device1")?;
        proxy
            .call_method("Disconnect", &())
            .map(|_| ())
            .map_err(Self::map_dbus_err)
    }

    fn read_value(&self, char_path: &str) -> Result<Vec<u8>, TransportError> {
        let proxy = self.proxy(char_path, "org.bluez.GattCharacteristic1")?;
        let reply = proxy
            .call_method(
                "ReadValue",
                &(HashMap::<String, zbus::zvariant::Value>::new(),),
            )
            .map_err(Self::map_dbus_err)?;
        let bytes: Vec<u8> = reply
            .body()
            .deserialize()
            .map_err(|e| TransportError::Bus(e.to_string()))?;
        Ok(bytes)
    }

    fn write_value(&self, char_path: &str, bytes: &[u8]) -> Result<(), TransportError> {
        let proxy = self.proxy(char_path, "org.bluez.GattCharacteristic1")?;
        let options = HashMap::<String, zbus::zvariant::Value>::new();
        proxy
            .call_method("WriteValue", &(bytes.to_vec(), options))
            .map(|_| ())
            .map_err(Self::map_dbus_err)
    }

    fn start_notify(&self, char_path: &str) -> Result<(), TransportError> {
        let proxy = self.proxy(char_path, "org.bluez.GattCharacteristic1")?;
        proxy
            .call_method("StartNotify", &())
            .map(|_| ())
            .map_err(Self::map_dbus_err)
    }

    fn stop_notify(&self, char_path: &str) -> Result<(), TransportError> {
        let proxy = self.proxy(char_path, "org.bluez.GattCharacteristic1")?;
        proxy
            .call_method("StopNotify", &())
            .map(|_| ())
            .map_err(Self::map_dbus_err)
    }

    fn active_hfp_transport(&self) -> Result<Option<String>, TransportError> {
        Ok(find_active_hfp_transport(&self.managed_objects()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcy_protocol::packet::encode_command;
    use std::cell::RefCell;
    use std::sync::{Arc, Mutex};

    type WriteLog = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

    #[derive(Default)]
    struct FakeBus {
        /// Shared, interior-mutable so tests can keep a handle and simulate
        /// host-side changes (link loss, SCO acquisition) after the bus is
        /// moved into the transport.
        objects: Arc<Mutex<Vec<BlueZObject>>>,
        connected: RefCell<bool>,
        writes: WriteLog,
        notifies: Arc<Mutex<Vec<String>>>,
        fail_connect: bool,
        /// Device object paths for which `Device1.Connect` fails.
        fail_connect_paths: RefCell<Vec<String>>,
        /// Device object paths for which `Device1.Connect` answers
        /// `org.bluez.Error.Failed: br-connection-busy` (BlueZ's answer when
        /// the link is already up/busy, e.g. audio already connected).
        busy_connect_paths: Vec<String>,
        /// Every path passed to `Device1.Connect`, in call order (shared so
        /// tests can assert on it after the bus is moved into the transport).
        connect_calls: Arc<Mutex<Vec<String>>>,
    }

    fn obj(path: &str, iface: &str, props: Vec<(&str, PropValue)>) -> BlueZObject {
        let mut map = HashMap::new();
        for (k, v) in props {
            map.insert(k.to_string(), v);
        }
        let mut interfaces = HashMap::new();
        interfaces.insert(iface.to_string(), map);
        BlueZObject {
            path: path.to_string(),
            interfaces,
        }
    }

    fn ht08_fixture() -> Vec<BlueZObject> {
        let dev = "/org/bluez/hci0/dev_F8_5C_7D_12_08_08";
        let svc = format!("{dev}/service0001");
        let char_cmd = format!("{svc}/char0001");
        let char_batt = format!("{svc}/char0002");
        vec![
            obj(
                dev,
                DEVICE_IFACE,
                vec![
                    ("Address", PropValue::Str("F8:5C:7D:12:08:08".into())),
                    ("Alias", PropValue::Str("QCY MeloBuds Pro".into())),
                    ("RSSI", PropValue::I16(-52)),
                ],
            ),
            obj(
                &svc,
                SERVICE_IFACE,
                vec![("UUID", PropValue::Str(SERVICE_MAIN.into()))],
            ),
            obj(
                &char_cmd,
                CHAR_IFACE,
                vec![(
                    "UUID",
                    PropValue::Str(crate::policy::CHAR_COMMAND_WRITE.into()),
                )],
            ),
            obj(
                &char_batt,
                CHAR_IFACE,
                vec![(
                    "UUID",
                    PropValue::Str("00000008-0000-1000-8000-00805f9b34fb".into()),
                )],
            ),
        ]
    }

    impl BlueZBus for FakeBus {
        fn managed_objects(&self) -> Result<Vec<BlueZObject>, TransportError> {
            Ok(self.objects.lock().expect("objects mutex").clone())
        }
        fn start_discovery(&self, _adapter: &str) -> Result<(), TransportError> {
            Ok(())
        }
        fn stop_discovery(&self, _adapter: &str) -> Result<(), TransportError> {
            Ok(())
        }
        fn device_connect(&self, p: &str) -> Result<(), TransportError> {
            self.connect_calls
                .lock()
                .expect("connect_calls mutex")
                .push(p.to_string());
            if self.fail_connect || self.fail_connect_paths.borrow().iter().any(|x| x == p) {
                return Err(TransportError::DeviceOutOfRange);
            }
            if self.busy_connect_paths.iter().any(|x| x == p) {
                return Err(TransportError::Bus(
                    "org.bluez.Error.Failed: br-connection-busy".into(),
                ));
            }
            *self.connected.borrow_mut() = true;
            Ok(())
        }
        fn device_disconnect(&self, _p: &str) -> Result<(), TransportError> {
            *self.connected.borrow_mut() = false;
            Ok(())
        }
        fn read_value(&self, char_path: &str) -> Result<Vec<u8>, TransportError> {
            if char_path.ends_with("char0002") {
                Ok(vec![0x52, 0x50, 0x5E])
            } else {
                Err(TransportError::NotFound(char_path.into()))
            }
        }
        fn write_value(&self, char_path: &str, bytes: &[u8]) -> Result<(), TransportError> {
            self.writes
                .lock()
                .expect("writes mutex")
                .push((char_path.to_string(), bytes.to_vec()));
            Ok(())
        }
        fn start_notify(&self, char_path: &str) -> Result<(), TransportError> {
            self.notifies
                .lock()
                .expect("notifies mutex")
                .push(char_path.to_string());
            Ok(())
        }
        fn stop_notify(&self, _p: &str) -> Result<(), TransportError> {
            Ok(())
        }
        fn active_hfp_transport(&self) -> Result<Option<String>, TransportError> {
            Ok(find_active_hfp_transport(
                &self.objects.lock().expect("objects mutex"),
            ))
        }
    }

    fn transport(objects: Vec<BlueZObject>) -> (BlueZTransport, WriteLog) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let bus = FakeBus {
            objects: Arc::new(Mutex::new(objects)),
            writes: writes.clone(),
            ..Default::default()
        };
        let mut t = BlueZTransport::new(Box::new(bus), WritePolicy::ht08());
        // Zero discovery windows: tests take instant snapshots, no real waits.
        t.scan_window = Duration::ZERO;
        t.le_fallback_window = Duration::ZERO;
        (t, writes)
    }

    /// Like [`transport`], but keeping a handle to the recorded `Connect` calls.
    fn transport_with_calls(
        objects: Vec<BlueZObject>,
    ) -> (BlueZTransport, WriteLog, Arc<Mutex<Vec<String>>>) {
        let writes = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bus = FakeBus {
            objects: Arc::new(Mutex::new(objects)),
            writes: writes.clone(),
            connect_calls: calls.clone(),
            ..Default::default()
        };
        let mut t = BlueZTransport::new(Box::new(bus), WritePolicy::ht08());
        t.scan_window = Duration::ZERO;
        t.le_fallback_window = Duration::ZERO;
        (t, writes, calls)
    }

    #[test]
    fn mac_normalization_and_device_path() {
        assert_eq!(normalize_mac("f8:5c:7d:12:08:08"), "F8_5C_7D_12_08_08");
        assert_eq!(
            device_path("hci0", "F8:5C:7D:12:08:08"),
            "/org/bluez/hci0/dev_F8_5C_7D_12_08_08"
        );
    }

    #[test]
    fn scan_filters_qcy_and_marks_model_known() {
        let mut objects = ht08_fixture();
        objects.push(obj(
            "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("AA:BB:CC:DD:EE:FF".into())),
                ("Alias", PropValue::Str("QCY T20".into())),
            ],
        ));
        objects.push(obj(
            "/org/bluez/hci0/dev_11_22_33_44_55_66",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("11:22:33:44:55:66".into())),
                ("Alias", PropValue::Str("Some Headphones".into())),
            ],
        ));
        let (mut t, _) = transport(objects);
        let list = t.scan().unwrap();
        assert_eq!(list.len(), 2); // non-QCY device filtered out
        let pro = list.iter().find(|d| d.name == "QCY MeloBuds Pro").unwrap();
        assert!(pro.model_known);
        let t20 = list.iter().find(|d| d.name == "QCY T20").unwrap();
        assert!(!t20.model_known); // unknown model stays read-only candidate
    }

    #[test]
    fn connect_resolves_characteristics_and_write_targets_command_char() {
        let (mut t, writes) = transport(ht08_fixture());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        t.write(&frame).unwrap();
        let w = writes.lock().expect("writes mutex");
        assert_eq!(w.len(), 1);
        assert!(w[0].0.ends_with("char0001"));
    }

    #[test]
    fn read_uses_resolved_characteristic_path() {
        let (mut t, _) = transport(ht08_fixture());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        let bytes = t.read("00000008-0000-1000-8000-00805f9b34fb").unwrap();
        assert_eq!(bytes, vec![0x52, 0x50, 0x5E]);
    }

    #[test]
    fn destructive_write_is_denied_before_reaching_the_bus() {
        let (mut t, writes) = transport(ht08_fixture());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        let frame = encode_command(0x01, &[]).unwrap();
        assert!(matches!(t.write(&frame), Err(TransportError::Denied(_))));
        assert!(writes.lock().expect("writes mutex").is_empty());
    }

    #[test]
    fn subscribe_starts_notify_on_the_resolved_char() {
        let (mut t, _) = transport(ht08_fixture());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        t.subscribe(crate::policy::CHAR_COMMAND_WRITE).unwrap();
    }

    fn unknown_qcy_fixture() -> Vec<BlueZObject> {
        let dev = "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF";
        let svc = format!("{dev}/service0001");
        let char_cmd = format!("{svc}/char0001");
        let char_batt = format!("{svc}/char0002");
        vec![
            obj(
                dev,
                DEVICE_IFACE,
                vec![
                    ("Address", PropValue::Str("AA:BB:CC:DD:EE:FF".into())),
                    ("Alias", PropValue::Str("QCY T20".into())),
                ],
            ),
            obj(
                &svc,
                SERVICE_IFACE,
                vec![("UUID", PropValue::Str(SERVICE_MAIN.into()))],
            ),
            obj(
                &char_cmd,
                CHAR_IFACE,
                vec![(
                    "UUID",
                    PropValue::Str(crate::policy::CHAR_COMMAND_WRITE.into()),
                )],
            ),
            obj(
                &char_batt,
                CHAR_IFACE,
                vec![(
                    "UUID",
                    PropValue::Str("00000008-0000-1000-8000-00805f9b34fb".into()),
                )],
            ),
        ]
    }

    #[test]
    fn unknown_model_device_is_read_only_after_connect() {
        let (mut t, writes) = transport(unknown_qcy_fixture());
        t.connect("AA:BB:CC:DD:EE:FF").unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        match t.write(&frame) {
            Err(TransportError::Denied(crate::policy::Denial::ReadOnlyDevice)) => {}
            other => panic!("expected ReadOnlyDevice denial, got {other:?}"),
        }
        assert!(writes.lock().expect("writes mutex").is_empty());
        // Read-only still permits characteristic reads.
        assert!(t.read("00000008-0000-1000-8000-00805f9b34fb").is_ok());
    }

    #[test]
    fn user_attestation_lifts_read_only_but_never_allows_destructive() {
        let (mut t, writes) = transport(unknown_qcy_fixture());
        t.connect("AA:BB:CC:DD:EE:FF").unwrap();
        t.attest_model_known();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        t.write(&frame).unwrap();
        assert_eq!(writes.lock().expect("writes mutex").len(), 1);
        // Destructive opcodes stay forbidden even after attestation.
        let reset = encode_command(0x01, &[]).unwrap();
        assert!(matches!(t.write(&reset), Err(TransportError::Denied(_))));
        assert_eq!(writes.lock().expect("writes mutex").len(), 1);
    }

    #[test]
    fn disconnect_clears_model_evidence() {
        let (mut t, writes) = transport(ht08_fixture());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        t.disconnect().unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        // After disconnect the device is both gone and no longer proven: nothing
        // may reach the bus either way.
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(_)) | Err(TransportError::Disconnected)
        ));
        assert!(writes.lock().expect("writes mutex").is_empty());
    }

    #[test]
    fn connect_failure_surfaces_structured_error() {
        let writes: WriteLog = Arc::new(Mutex::new(Vec::new()));
        let bus = FakeBus {
            objects: Arc::new(Mutex::new(ht08_fixture())),
            writes: writes.clone(),
            fail_connect: true,
            ..Default::default()
        };
        let mut t = BlueZTransport::new(Box::new(bus), WritePolicy::ht08());
        t.scan_window = Duration::ZERO;
        t.le_fallback_window = Duration::ZERO;
        assert_eq!(
            t.connect("F8:5C:7D:12:08:08"),
            Err(TransportError::DeviceOutOfRange)
        );
        // Initial-connect failure leaves no session behind: I/O reports
        // disconnected and nothing reaches the bus.
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert_eq!(t.write(&frame), Err(TransportError::Disconnected));
        assert_eq!(
            t.read("00000008-0000-1000-8000-00805f9b34fb"),
            Err(TransportError::Disconnected)
        );
        assert!(writes.lock().expect("writes mutex").is_empty());
    }

    /// Two fully resolved known-model devices with different names, so the
    /// dual-mode fallback can never confuse them with each other.
    fn two_device_fixture() -> Vec<BlueZObject> {
        let mut objects = ht08_fixture();
        let dev_b = "/org/bluez/hci0/dev_11_22_33_44_55_66";
        let svc_b = format!("{dev_b}/service0001");
        objects.push(obj(
            dev_b,
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("11:22:33:44:55:66".into())),
                ("Alias", PropValue::Str("QCY HT08".into())),
                ("RSSI", PropValue::I16(-40)),
            ],
        ));
        objects.push(obj(
            &svc_b,
            SERVICE_IFACE,
            vec![("UUID", PropValue::Str(SERVICE_MAIN.into()))],
        ));
        objects.push(obj(
            &format!("{svc_b}/char0001"),
            CHAR_IFACE,
            vec![(
                "UUID",
                PropValue::Str(crate::policy::CHAR_COMMAND_WRITE.into()),
            )],
        ));
        objects.push(obj(
            &format!("{svc_b}/char0002"),
            CHAR_IFACE,
            vec![(
                "UUID",
                PropValue::Str("00000008-0000-1000-8000-00805f9b34fb".into()),
            )],
        ));
        objects
    }

    #[test]
    fn connect_replaces_the_previous_device_session() {
        let (mut t, writes) = transport(two_device_fixture());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        t.write(&frame).unwrap();
        // Replace device A with device B.
        t.connect("11:22:33:44:55:66").unwrap();
        t.write(&frame).unwrap();
        let w = writes.lock().expect("writes mutex");
        assert_eq!(w.len(), 2);
        assert!(w[0].0.contains("dev_F8_5C_7D_12_08_08"));
        // After replacement, writes must target B's characteristics only.
        assert!(w[1].0.contains("dev_11_22_33_44_55_66"));
        // Reads also go through B's resolved characteristics.
        let bytes = t.read("00000008-0000-1000-8000-00805f9b34fb").unwrap();
        assert_eq!(bytes, vec![0x52, 0x50, 0x5E]);
    }

    #[test]
    fn failed_connect_invalidates_the_previous_session() {
        // A is fully connected; B fails at the D-Bus connect level. Different
        // names and no vendor UUIDs keep the dual-mode fallback from silently
        // re-connecting A as B's "identity".
        let writes: WriteLog = Arc::new(Mutex::new(Vec::new()));
        let bus = FakeBus {
            objects: Arc::new(Mutex::new(two_device_fixture())),
            writes: writes.clone(),
            fail_connect_paths: RefCell::new(vec![device_path("hci0", "11:22:33:44:55:66")]),
            ..Default::default()
        };
        let mut t = BlueZTransport::new(Box::new(bus), WritePolicy::ht08());
        t.scan_window = Duration::ZERO;
        t.le_fallback_window = Duration::ZERO;

        t.connect("F8:5C:7D:12:08:08").unwrap();
        assert_eq!(
            t.connect("11:22:33:44:55:66"),
            Err(TransportError::DeviceOutOfRange)
        );
        // Session fully invalidated: no I/O may reach A's stale char paths.
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert_eq!(t.write(&frame), Err(TransportError::Disconnected));
        assert_eq!(
            t.read("00000008-0000-1000-8000-00805f9b34fb"),
            Err(TransportError::Disconnected)
        );
        assert_eq!(
            t.subscribe(crate::policy::CHAR_COMMAND_WRITE),
            Err(TransportError::Disconnected)
        );
        assert!(writes.lock().expect("writes mutex").is_empty());
    }

    #[test]
    fn failed_service_resolution_invalidates_the_previous_session() {
        // A is fully resolved; B is a known model whose D-Bus connect succeeds
        // but whose GATT service resolution fails (no vendor service objects).
        let mut objects = ht08_fixture();
        objects.push(obj(
            "/org/bluez/hci0/dev_11_22_33_44_55_66",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("11:22:33:44:55:66".into())),
                ("Alias", PropValue::Str("QCY HT08".into())),
            ],
        ));
        let (mut t, writes) = transport(objects);
        t.connect("F8:5C:7D:12:08:08").unwrap();
        assert!(t.connect("11:22:33:44:55:66").is_err());
        // No half-bound state: B's path must never be usable, and A's cached
        // characteristics must not survive the failed replacement.
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert_eq!(t.write(&frame), Err(TransportError::Disconnected));
        assert_eq!(
            t.read("00000008-0000-1000-8000-00805f9b34fb"),
            Err(TransportError::Disconnected)
        );
        assert!(writes.lock().expect("writes mutex").is_empty());
    }

    /// Dual-mode fixture: the BR/EDR identity (audio pairing, no GATT children)
    /// plus the BLE identity (same renamed name, vendor GATT service).
    fn dual_mode_fixture() -> Vec<BlueZObject> {
        let mut objects = Vec::new();
        let bredr = "/org/bluez/hci0/dev_84_AC_60_62_69_DA";
        objects.push(obj(
            bredr,
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("84:AC:60:62:69:DA".into())),
                ("Alias", PropValue::Str("MeloBuds de Carol".into())),
            ],
        ));
        let le = "/org/bluez/hci0/dev_C4_AC_60_62_69_DB";
        let svc = format!("{le}/service0001");
        let char_cmd = format!("{svc}/char0001");
        let char_batt = format!("{svc}/char0002");
        objects.push(obj(
            le,
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("C4:AC:60:62:69:DB".into())),
                ("Alias", PropValue::Str("MeloBuds de Carol".into())),
                ("RSSI", PropValue::I16(-60)),
            ],
        ));
        objects.push(obj(
            &svc,
            SERVICE_IFACE,
            vec![("UUID", PropValue::Str(SERVICE_MAIN.into()))],
        ));
        objects.push(obj(
            &char_cmd,
            CHAR_IFACE,
            vec![(
                "UUID",
                PropValue::Str(crate::policy::CHAR_COMMAND_WRITE.into()),
            )],
        ));
        objects.push(obj(
            &char_batt,
            CHAR_IFACE,
            vec![(
                "UUID",
                PropValue::Str("00000008-0000-1000-8000-00805f9b34fb".into()),
            )],
        ));
        objects
    }

    #[test]
    fn connect_falls_back_to_the_ble_identity_of_a_dual_mode_device() {
        let (mut t, writes) = transport(dual_mode_fixture());
        // The user picks the BR/EDR address (the paired audio device); the
        // transport must bridge to the BLE identity that carries the GATT.
        t.connect("84:AC:60:62:69:DA").unwrap();
        // Renamed device: model still unproven, so writes stay denied...
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice
            ))
        ));
        // ...until the user attests the model; then writes target the BLE char.
        t.attest_model_known();
        t.write(&frame).unwrap();
        let w = writes.lock().expect("writes mutex");
        assert_eq!(w.len(), 1);
        assert!(w[0].0.contains("dev_C4_AC_60_62_69_DB"));
        // Status reads work through the BLE identity too.
        assert!(t.read("00000008-0000-1000-8000-00805f9b34fb").is_ok());
    }

    #[test]
    fn connect_fallback_accepts_a_vendor_service_advertisement() {
        // The BLE identity has a different name but advertises the vendor main
        // service in its UUIDs — accepted as a fallback candidate.
        let mut objects = Vec::new();
        objects.push(obj(
            "/org/bluez/hci0/dev_84_AC_60_62_69_DA",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("84:AC:60:62:69:DA".into())),
                ("Alias", PropValue::Str("MeloBuds de Carol".into())),
            ],
        ));
        let le = "/org/bluez/hci0/dev_C4_AC_60_62_69_DB";
        let svc = format!("{le}/service0001");
        objects.push(obj(
            le,
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("C4:AC:60:62:69:DB".into())),
                ("Alias", PropValue::Str("QCY-Buds".into())),
                ("UUIDs", PropValue::StrArray(vec![SERVICE_MAIN.to_string()])),
            ],
        ));
        objects.push(obj(
            &svc,
            SERVICE_IFACE,
            vec![("UUID", PropValue::Str(SERVICE_MAIN.into()))],
        ));
        objects.push(obj(
            &format!("{svc}/char0001"),
            CHAR_IFACE,
            vec![(
                "UUID",
                PropValue::Str(crate::policy::CHAR_COMMAND_WRITE.into()),
            )],
        ));
        let (mut t, _writes) = transport(objects);
        t.connect("84:AC:60:62:69:DA").unwrap();
    }

    #[test]
    fn connect_without_gatt_or_candidates_reports_actionable_error() {
        // Only the BR/EDR object exists and nothing else qualifies as a BLE
        // identity: the error must tell the user how to wake the BLE side.
        let objects = vec![obj(
            "/org/bluez/hci0/dev_84_AC_60_62_69_DA",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("84:AC:60:62:69:DA".into())),
                ("Alias", PropValue::Str("MeloBuds de Carol".into())),
            ],
        )];
        let (mut t, _writes) = transport(objects);
        match t.connect("84:AC:60:62:69:DA") {
            Err(TransportError::NotFound(msg)) => {
                assert!(msg.contains("charging case"), "message was: {msg}");
            }
            other => panic!("expected actionable NotFound, got {other:?}"),
        }
    }

    /* Already-connected attach (user report: `br-connection-busy`) */

    fn ht08_fixture_connected() -> Vec<BlueZObject> {
        // The standard HT08 fixture, but the device object reports an existing
        // host connection (e.g. the earbuds are connected for BR/EDR audio).
        let mut objects = ht08_fixture();
        let dev = objects
            .iter_mut()
            .find(|o| o.interfaces.contains_key(DEVICE_IFACE) && !o.path.contains("service"))
            .expect("device object");
        dev.interfaces
            .get_mut(DEVICE_IFACE)
            .unwrap()
            .insert("Connected".to_string(), PropValue::Bool(true));
        objects
    }

    #[test]
    fn connect_skips_redundant_connect_for_an_already_connected_device() {
        // The earbuds are already connected at the host level; `Connect()` must
        // not be issued again (BlueZ would answer br-connection-busy), and the
        // characteristics still resolve.
        let (mut t, _writes, calls) = transport_with_calls(ht08_fixture_connected());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        assert!(
            calls.lock().unwrap().is_empty(),
            "no Connect call expected for an already-connected device"
        );
        // The session is usable.
        assert!(t.read("00000008-0000-1000-8000-00805f9b34fb").is_ok());
    }

    #[test]
    fn connect_tolerates_br_connection_busy_when_the_link_is_already_up() {
        // Race variant: the object is not marked Connected when checked, but
        // BlueZ still answers br-connection-busy to Connect. The transport
        // proceeds to resolution instead of failing.
        let dev_path = device_path("hci0", "F8:5C:7D:12:08:08");
        let writes: WriteLog = Arc::new(Mutex::new(Vec::new()));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let bus = FakeBus {
            objects: Arc::new(Mutex::new(ht08_fixture())),
            writes: writes.clone(),
            busy_connect_paths: vec![dev_path.clone()],
            connect_calls: calls.clone(),
            ..Default::default()
        };
        let mut t = BlueZTransport::new(Box::new(bus), WritePolicy::ht08());
        t.scan_window = Duration::ZERO;
        t.le_fallback_window = Duration::ZERO;
        t.connect("F8:5C:7D:12:08:08").unwrap();
        assert_eq!(*calls.lock().unwrap(), vec![dev_path]);
        assert!(t.read("00000008-0000-1000-8000-00805f9b34fb").is_ok());
    }

    #[test]
    fn connected_devices_lists_only_connected_qcy_candidates() {
        let connected_qcy = obj(
            "/org/bluez/hci0/dev_F8_5C_7D_12_08_08",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("F8:5C:7D:12:08:08".into())),
                ("Alias", PropValue::Str("QCY MeloBuds Pro".into())),
                ("Connected", PropValue::Bool(true)),
            ],
        );
        // Renamed device, connected, exposing the vendor main service: listed,
        // but the model stays unproven.
        let connected_renamed = obj(
            "/org/bluez/hci0/dev_84_AC_60_62_69_DA",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("84:AC:60:62:69:DA".into())),
                ("Alias", PropValue::Str("Fones da Carol".into())),
                ("Connected", PropValue::Bool(true)),
                ("UUIDs", PropValue::StrArray(vec![SERVICE_MAIN.to_string()])),
            ],
        );
        // Connected but not QCY-ish: not listed.
        let connected_other = obj(
            "/org/bluez/hci0/dev_11_22_33_44_55_66",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("11:22:33:44:55:66".into())),
                ("Alias", PropValue::Str("Generic Speaker".into())),
                ("Connected", PropValue::Bool(true)),
            ],
        );
        // QCY-ish but not connected: not listed (a scan would find it).
        let disconnected_qcy = obj(
            "/org/bluez/hci0/dev_AA_BB_CC_DD_EE_FF",
            DEVICE_IFACE,
            vec![
                ("Address", PropValue::Str("AA:BB:CC:DD:EE:FF".into())),
                ("Alias", PropValue::Str("QCY MeloBuds Pro".into())),
                ("Connected", PropValue::Bool(false)),
            ],
        );
        let (mut t, _writes) = transport(vec![
            connected_qcy,
            connected_renamed,
            connected_other,
            disconnected_qcy,
        ]);
        let list = t.connected_devices().unwrap();
        let addrs: Vec<&str> = list.iter().map(|d| d.address.as_str()).collect();
        assert_eq!(addrs, vec!["84:AC:60:62:69:DA", "F8:5C:7D:12:08:08"]);
        let renamed = &list[0];
        assert!(!renamed.model_known, "renamed device stays unproven");
        let named = &list[1];
        assert!(named.model_known, "HT08 name proves the model");
    }

    #[test]
    fn connected_devices_is_empty_without_connected_devices() {
        let (mut t, _writes) = transport(ht08_fixture());
        assert!(t.connected_devices().unwrap().is_empty());
    }

    /// MediaTransport1 object fixture: an SCO (HFP) transport on the BR/EDR
    /// audio identity, as BlueZ exposes it while a call/mic session is held.
    fn hfp_transport_obj(state: &str) -> BlueZObject {
        obj(
            "/org/bluez/hci0/dev_84_AC_60_62_69_DA/fd0",
            TRANSPORT_IFACE,
            vec![
                (
                    "UUID",
                    PropValue::Str("0000111f-0000-1000-8000-00805f9b34fb".into()),
                ),
                ("State", PropValue::Str(state.into())),
                (
                    "Device",
                    PropValue::Str("/org/bluez/hci0/dev_84_AC_60_62_69_DA".into()),
                ),
            ],
        )
    }

    #[test]
    fn active_hfp_transport_blocks_the_le_fallback_with_an_actionable_error() {
        // Live HT08 evidence (#52): while SCO is held, LE connects are aborted
        // by the host. The connect path must fail fast with HfpBlocked instead
        // of burning the discovery window on attempts that cannot succeed.
        let mut objects = ht08_fixture();
        // Remove GATT so the primary connect fails and the LE fallback is the
        // only way forward.
        objects.retain(|o| !o.path.contains("service"));
        objects.push(hfp_transport_obj("active"));
        let (mut t, _writes, calls) = transport_with_calls(objects);
        let err = t.connect("F8:5C:7D:12:08:08").unwrap_err();
        match err {
            TransportError::HfpBlocked(detail) => {
                assert!(detail.contains("dev_84_AC_60_62_69_DA"), "{detail}");
            }
            other => panic!("expected HfpBlocked, got {other:?}"),
        }
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls.len(),
            1,
            "only the primary Connect may happen; the LE fallback must not start while HFP blocks"
        );
    }

    #[test]
    fn pending_hfp_transport_also_blocks() {
        let mut objects = ht08_fixture();
        objects.retain(|o| !o.path.contains("service"));
        objects.push(hfp_transport_obj("pending"));
        let (mut t, _writes, _calls) = transport_with_calls(objects);
        assert!(matches!(
            t.connect("F8:5C:7D:12:08:08"),
            Err(TransportError::HfpBlocked(_))
        ));
    }

    #[test]
    fn idle_hfp_and_a2dp_transports_do_not_block() {
        // SCO released (idle) or music-only (A2DP sink) must not block LE.
        let mut objects = ht08_fixture();
        objects.push(hfp_transport_obj("idle"));
        objects.push(obj(
            "/org/bluez/hci0/dev_84_AC_60_62_69_DA/fd1",
            TRANSPORT_IFACE,
            vec![
                (
                    "UUID",
                    PropValue::Str("0000110b-0000-1000-8000-00805f9b34fb".into()),
                ),
                ("State", PropValue::Str("active".into())),
                (
                    "Device",
                    PropValue::Str("/org/bluez/hci0/dev_84_AC_60_62_69_DA".into()),
                ),
            ],
        ));
        let (mut t, _writes) = transport(objects);
        t.connect("F8:5C:7D:12:08:08").unwrap();
    }

    #[test]
    fn is_connected_tracks_the_device_link_state() {
        let writes: WriteLog = Arc::new(Mutex::new(Vec::new()));
        let bus = FakeBus {
            objects: Arc::new(Mutex::new(ht08_fixture_connected())),
            writes: writes.clone(),
            ..Default::default()
        };
        let objects = bus.objects.clone();
        let mut t = BlueZTransport::new(Box::new(bus), WritePolicy::ht08());
        t.scan_window = Duration::ZERO;
        t.le_fallback_window = Duration::ZERO;
        t.connect("F8:5C:7D:12:08:08").unwrap();
        assert!(t.is_connected().unwrap(), "session is live after connect");

        // Simulate link loss: the device object flips Connected to false.
        {
            let mut objs = objects.lock().expect("objects mutex");
            let dev = objs
                .iter_mut()
                .find(|o| o.interfaces.contains_key(DEVICE_IFACE))
                .expect("device object");
            dev.interfaces
                .get_mut(DEVICE_IFACE)
                .unwrap()
                .insert("Connected".to_string(), PropValue::Bool(false));
        }
        assert!(
            !t.is_connected().unwrap(),
            "link loss must be visible to the session supervisor"
        );
    }

    #[test]
    fn is_connected_is_false_before_connect_and_after_disconnect() {
        let (mut t, _writes) = transport(ht08_fixture_connected());
        assert!(!t.is_connected().unwrap());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        assert!(t.is_connected().unwrap());
        t.disconnect().unwrap();
        assert!(!t.is_connected().unwrap());
    }
}
