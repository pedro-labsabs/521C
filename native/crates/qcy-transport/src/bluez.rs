//! BlueZ D-Bus transport backend (issue #7).
//!
//! Talks to the system BlueZ stack through its D-Bus GATT API — it never shells out to
//! interactive tools, never requires root, and never reconfigures the daemon. The D-Bus
//! access is isolated behind the [`BlueZBus`] trait so the object-mapping, discovery,
//! characteristic-resolution and policy logic can all be unit-tested against a fake bus
//! when no Bluetooth daemon is available.

use std::collections::HashMap;

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
}

/// A BlueZ object: its path plus per-interface property maps.
#[derive(Debug, Clone, Default)]
pub struct BlueZObject {
    pub path: String,
    pub interfaces: HashMap<String, HashMap<String, PropValue>>,
}

/// Minimal D-Bus surface the BlueZ backend needs. Implemented for real by
/// [`ZbusBlueZBus`] and faked in unit tests.
pub trait BlueZBus {
    fn managed_objects(&self) -> Result<Vec<BlueZObject>, TransportError>;
    fn start_discovery(&self, adapter: &str) -> Result<(), TransportError>;
    fn device_connect(&self, device_path: &str) -> Result<(), TransportError>;
    fn device_disconnect(&self, device_path: &str) -> Result<(), TransportError>;
    fn read_value(&self, char_path: &str) -> Result<Vec<u8>, TransportError>;
    fn write_value(&self, char_path: &str, bytes: &[u8]) -> Result<(), TransportError>;
    fn start_notify(&self, char_path: &str) -> Result<(), TransportError>;
    fn stop_notify(&self, char_path: &str) -> Result<(), TransportError>;
}

/* ------------------------------------------------------------------ */
/* BlueZ transport                                                     */
/* ------------------------------------------------------------------ */

const DEVICE_IFACE: &str = "org.bluez.Device1";
const SERVICE_IFACE: &str = "org.bluez.GattService1";
const CHAR_IFACE: &str = "org.bluez.GattCharacteristic1";

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
}

impl BlueZTransport {
    pub fn new(bus: Box<dyn BlueZBus>, policy: WritePolicy) -> Self {
        Self {
            bus,
            policy,
            experimental_opt_in: false,
            adapter: "hci0".to_string(),
            device_path: None,
            chars: HashMap::new(),
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

    fn char_path(&self, char_uuid: &str) -> Result<String, TransportError> {
        self.chars
            .get(&char_uuid.to_ascii_lowercase())
            .cloned()
            .ok_or_else(|| TransportError::NotFound(char_uuid.to_string()))
    }
}

impl Transport for BlueZTransport {
    fn scan(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        self.bus.start_discovery(&self.adapter)?;
        let objects = self.bus.managed_objects()?;
        let mut out = Vec::new();
        for obj in &objects {
            if !obj.interfaces.contains_key(DEVICE_IFACE) {
                continue;
            }
            let name = Self::prop_str(obj, DEVICE_IFACE, "Alias")
                .or_else(|| Self::prop_str(obj, DEVICE_IFACE, "Name"))
                .unwrap_or("");
            if !Self::is_qcy_name(name) {
                continue;
            }
            let address = Self::prop_str(obj, DEVICE_IFACE, "Address").unwrap_or("");
            let rssi = match obj.interfaces.get(DEVICE_IFACE).and_then(|p| p.get("RSSI")) {
                Some(PropValue::I16(v)) => Some(*v),
                _ => None,
            };
            out.push(DiscoveredDevice {
                address: address.to_string(),
                name: name.to_string(),
                rssi,
                model_known: Self::is_known_model(name),
            });
        }
        Ok(out)
    }

    fn connect(&mut self, address: &str) -> Result<(), TransportError> {
        let path = device_path(&self.adapter, address);
        self.bus.device_connect(&path)?;
        self.device_path = Some(path);
        self.resolve_chars()?;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        if let Some(path) = self.device_path.take() {
            self.chars.clear();
            self.bus.device_disconnect(&path)?;
        }
        Ok(())
    }

    fn read(&mut self, char_uuid: &str) -> Result<Vec<u8>, TransportError> {
        let path = self.char_path(char_uuid)?;
        self.bus.read_value(&path)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.policy
            .authorize_frame(bytes, self.experimental_opt_in)
            .map_err(TransportError::Denied)?;
        let path = self.char_path(crate::policy::CHAR_COMMAND_WRITE)?;
        self.bus.write_value(&path, bytes)
    }

    fn write_direct(&mut self, char_uuid: &str, bytes: &[u8]) -> Result<(), TransportError> {
        self.policy
            .authorize_direct(char_uuid, bytes, self.experimental_opt_in)
            .map_err(TransportError::Denied)?;
        let path = self.char_path(char_uuid)?;
        self.bus.write_value(&path, bytes)
    }

    fn subscribe(&mut self, char_uuid: &str) -> Result<(), TransportError> {
        let path = self.char_path(char_uuid)?;
        self.bus.start_notify(&path)
    }

    fn set_experimental_opt_in(&mut self, on: bool) {
        self.experimental_opt_in = on;
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
                bytes.ok().map(PropValue::Bytes)
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcy_protocol::packet::encode_command;
    use std::cell::RefCell;
    use std::rc::Rc;

    type WriteLog = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

    #[derive(Default)]
    struct FakeBus {
        objects: Vec<BlueZObject>,
        connected: RefCell<bool>,
        writes: WriteLog,
        notifies: Rc<RefCell<Vec<String>>>,
        fail_connect: bool,
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
            Ok(self.objects.clone())
        }
        fn start_discovery(&self, _adapter: &str) -> Result<(), TransportError> {
            Ok(())
        }
        fn device_connect(&self, _p: &str) -> Result<(), TransportError> {
            if self.fail_connect {
                return Err(TransportError::DeviceOutOfRange);
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
                .borrow_mut()
                .push((char_path.to_string(), bytes.to_vec()));
            Ok(())
        }
        fn start_notify(&self, char_path: &str) -> Result<(), TransportError> {
            self.notifies.borrow_mut().push(char_path.to_string());
            Ok(())
        }
        fn stop_notify(&self, _p: &str) -> Result<(), TransportError> {
            Ok(())
        }
    }

    fn transport(objects: Vec<BlueZObject>) -> (BlueZTransport, WriteLog) {
        let writes = Rc::new(RefCell::new(Vec::new()));
        let bus = FakeBus {
            objects,
            writes: writes.clone(),
            ..Default::default()
        };
        (
            BlueZTransport::new(Box::new(bus), WritePolicy::ht08()),
            writes,
        )
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
        let w = writes.borrow();
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
        assert!(writes.borrow().is_empty());
    }

    #[test]
    fn subscribe_starts_notify_on_the_resolved_char() {
        let (mut t, _) = transport(ht08_fixture());
        t.connect("F8:5C:7D:12:08:08").unwrap();
        t.subscribe(crate::policy::CHAR_COMMAND_WRITE).unwrap();
    }

    #[test]
    fn connect_failure_surfaces_structured_error() {
        let bus = FakeBus {
            objects: ht08_fixture(),
            fail_connect: true,
            ..Default::default()
        };
        let mut t = BlueZTransport::new(Box::new(bus), WritePolicy::ht08());
        assert_eq!(
            t.connect("F8:5C:7D:12:08:08"),
            Err(TransportError::DeviceOutOfRange)
        );
    }
}
