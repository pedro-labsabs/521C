//! SPP/RFCOMM transport backend (issue #50, retargeted by #52).
//!
//! **Not the HT08 control path.** Live validation (#52) proved the HT08
//! control surface is BLE GATT: BlueZ resolves the vendor service
//! (`0000a001`) on the earbuds' separate LE control identity, and HT08 SPP
//! channel 1 ("COM5") only byte-ACKs frames without executing them (see
//! `docs/devices/HT08.md`). This backend remains as a **generic** transport
//! for QCY models whose evidence may point at RFCOMM, carrying the same
//! `0xFF`-framed command protocol this crate implements — only the byte pipe
//! changes; the codecs and the write policy are reused unchanged.
//!
//! This backend opens a raw `AF_BLUETOOTH` / `BTPROTO_RFCOMM` socket to the
//! device's BR/EDR address. It never requires root, never shells out, and
//! never reconfigures the Bluetooth daemon. Socket I/O is isolated behind the
//! [`RfcommSocketFactory`] trait so framing, read mapping and policy logic are
//! unit-tested against a scripted fake without hardware, mirroring the
//! [`crate::bluez::BlueZBus`] pattern.
//!
//! Trust model: bytes arriving from the earbuds are untrusted input. Every
//! frame is validated by [`qcy_protocol::packet::decode_packet`] before use,
//! garbage between frames is discarded, buffer growth is bounded, and reads
//! time out. Outbound writes pass the central [`WritePolicy`] exactly like the
//! GATT backend: destructive opcodes are never issued and unknown devices stay
//! read-only until explicit user attestation.
//!
//! Characteristic mapping: SPP has no GATT characteristics. Status reads are
//! performed as `RequestData(0xFE)` exchanges over the stream — the policy
//! authorizes `0xFE` even for read-only devices because it is a read-back
//! request, not a state mutation (same rule as the TypeScript policy).
//! [`Transport::write_direct`] has no SPP equivalent and reports the
//! characteristic as not found.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use qcy_protocol::packet::{decode_packet, encode_command};
use qcy_protocol::SOF;

use crate::policy::{WritePolicy, CHAR_SETTINGS_NOTIFY};
use crate::{normalize_address, DiscoveredDevice, Transport, TransportError};

/// SPP service UUID (Serial Port Profile) as cached by BlueZ for these devices.
pub const SPP_UUID: &str = "00001101-0000-1000-8000-00805f9b34fb";

/// Default RFCOMM channel for the QCY control service. Corroborated by
/// independent Jieli-earbud projects referenced in issue #50; HT08-specific
/// confirmation belongs in the evidence ledger (SDP query on real hardware).
pub const DEFAULT_RFCOMM_CHANNEL: u8 = 1;

/// RequestData opcode: read-back request used to read state over the stream.
const REQUEST_DATA: u8 = 0xFE;
/// Opcode the device answers with after `RequestData(0xFE, [0x2F])`.
const OP_BATTERY: u8 = 0x2F;
/// Opcode the device answers with after `RequestData(0xFE, [0x30])`.
const OP_VERSION: u8 = 0x30;

/// Battery GATT characteristic UUID, mapped to a stream `0x2F` read.
const CHAR_BATTERY: &str = "00000008-0000-1000-8000-00805f9b34fb";
/// Version GATT characteristic UUID, mapped to a stream `0x30` read.
const CHAR_VERSION: &str = "00000007-0000-1000-8000-00805f9b34fb";

/// A valid frame is at most `2 + 255` bytes (one-byte declared body length).
const MAX_FRAME: usize = 257;
/// Plausibility bound used for stream resynchronization. Every documented
/// QCY frame (corpus + live captures) is far smaller than this; a candidate
/// whose declared length exceeds it is treated as a stray `0xFF` (e.g. a
/// garbage byte or a truncated frame boundary) and the reader resyncs on the
/// next SOF instead of stalling while waiting for up to 257 bytes that will
/// never form a valid frame (audit #68).
const MAX_REASONABLE_FRAME: usize = 128;
/// Bound on the reassembly buffer; a garbage stream cannot grow it past this.
const MAX_BUFFER: usize = 4096;
/// Bound on queued unsolicited notification frames (oldest dropped).
const MAX_NOTIFY_QUEUE: usize = 64;
/// Default window for a read-back response to arrive.
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(3);

/* ------------------------------------------------------------------ */
/* Stream frame extraction (pure, deterministic, unit-tested)          */
/* ------------------------------------------------------------------ */

/// Incremental extractor of validated `0xFF` frames from the SPP byte stream.
///
/// The peer is untrusted input: the reader tolerates garbage between frames,
/// reassembles frames split across socket reads, drops candidates that fail
/// [`decode_packet`], and bounds its buffer so a hostile or buggy peer cannot
/// grow memory without limit.
#[derive(Debug, Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append bytes received from the wire.
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
        if self.buf.len() > MAX_BUFFER {
            let excess = self.buf.len() - MAX_BUFFER;
            self.buf.drain(..excess);
        }
        self.skip_to_sof();
    }

    fn skip_to_sof(&mut self) {
        match self.buf.iter().position(|b| *b == SOF) {
            Some(0) => {}
            Some(pos) => {
                self.buf.drain(..pos);
            }
            None => self.buf.clear(),
        }
    }

    /// Extract the next validated frame, or `None` when more bytes are needed.
    /// Invalid candidates are discarded and scanning resumes after them.
    pub fn next_frame(&mut self) -> Option<Vec<u8>> {
        loop {
            self.skip_to_sof();
            if self.buf.len() < 2 {
                return None;
            }
            let total = self.buf[1] as usize + 2;
            debug_assert!(total <= MAX_FRAME);
            if total > MAX_REASONABLE_FRAME {
                // Implausible declared length: this SOF is stray data. Drop it
                // and rescan instead of waiting for a huge frame that would
                // stall the reader and swallow any valid frame behind it.
                self.buf.drain(..1);
                continue;
            }
            if self.buf.len() < total {
                return None; // incomplete frame: wait for more bytes
            }
            let candidate = self.buf[..total].to_vec();
            if decode_packet(&candidate).is_ok() {
                self.buf.drain(..total);
                return Some(candidate);
            }
            // Complete but invalid candidate: drop its SOF and rescan.
            self.buf.drain(..1);
        }
    }
}

/* ------------------------------------------------------------------ */
/* Fakeable socket boundary                                            */
/* ------------------------------------------------------------------ */

/// A connected RFCOMM byte pipe. Implemented for real by [`RawRfcommSocket`]
/// and faked in unit tests.
pub trait RfcommSocket: Send {
    /// Blocking read bounded by the socket's configured receive timeout.
    /// `Ok(0)` means the peer closed the connection; `Err(TransportError::Timeout)`
    /// means the receive timeout elapsed with no data (callers retry until
    /// their own deadline).
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError>;
    /// Write all bytes to the pipe.
    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
}

/// Opens an RFCOMM connection to `(address, channel)`.
pub trait RfcommSocketFactory: Send {
    fn open(&self, address: &str, channel: u8) -> Result<Box<dyn RfcommSocket>, TransportError>;
}

/* ------------------------------------------------------------------ */
/* SPP/RFCOMM transport                                                */
/* ------------------------------------------------------------------ */

/// Blocking, per-connection SPP/RFCOMM transport behind the [`Transport`]
/// contract. Reads are `RequestData(0xFE)` exchanges; writes are policy-checked
/// framed commands; all inbound bytes are validated before use.
pub struct RfcommTransport {
    factory: Box<dyn RfcommSocketFactory>,
    policy: WritePolicy,
    experimental_opt_in: bool,
    channel: u8,
    socket: Option<Box<dyn RfcommSocket>>,
    connected_address: Option<String>,
    /// Model evidence for the connected device. SPP exposes no advertised
    /// name, so the device starts unknown/read-only and only becomes writable
    /// through explicit user attestation (or a previously confirmed address,
    /// which the application layer turns into an attestation).
    connected_model_known: bool,
    reader: FrameReader,
    /// Validated unsolicited frames received while answering reads.
    notify_queue: VecDeque<Vec<u8>>,
    subscribed: bool,
    /// Response window for a read-back exchange (injectable for tests).
    pub(crate) response_timeout: Duration,
}

impl RfcommTransport {
    pub fn new(factory: Box<dyn RfcommSocketFactory>, policy: WritePolicy) -> Self {
        Self {
            factory,
            policy,
            experimental_opt_in: false,
            channel: DEFAULT_RFCOMM_CHANNEL,
            socket: None,
            connected_address: None,
            connected_model_known: false,
            reader: FrameReader::new(),
            notify_queue: VecDeque::new(),
            subscribed: false,
            response_timeout: RESPONSE_TIMEOUT,
        }
    }

    /// Use a specific RFCOMM channel instead of the default (1).
    pub fn with_channel(mut self, channel: u8) -> Self {
        self.channel = channel;
        self
    }

    /// Override the read-back response window (tests; default 3 s).
    pub fn with_response_timeout(mut self, timeout: Duration) -> Self {
        self.response_timeout = timeout;
        self
    }

    /// Address of the connected device, if any.
    pub fn connected_address(&self) -> Option<&str> {
        self.connected_address.as_deref()
    }

    /// Pop the oldest queued unsolicited frame, if any.
    pub fn pop_notify(&mut self) -> Option<Vec<u8>> {
        self.notify_queue.pop_front()
    }

    fn queue_notify(&mut self, frame: Vec<u8>) {
        // Mirror GATT semantics: unsolicited frames are only surfaced as
        // notifications after an explicit [`Transport::subscribe`] on the
        // settings-notify characteristic. Frames arriving while answering a
        // read are dropped otherwise (they were not requested and nobody is
        // listening for them).
        if !self.subscribed {
            return;
        }
        if self.notify_queue.len() >= MAX_NOTIFY_QUEUE {
            self.notify_queue.pop_front();
        }
        self.notify_queue.push_back(frame);
    }

    /// Map a GATT read characteristic to the opcode answered over the stream.
    fn read_opcode(char_uuid: &str) -> Option<u8> {
        let uuid = char_uuid.to_ascii_lowercase();
        if uuid == CHAR_BATTERY {
            Some(OP_BATTERY)
        } else if uuid == CHAR_VERSION {
            Some(OP_VERSION)
        } else {
            None
        }
    }

    /// Wait for a validated frame carrying `op`, queueing everything else.
    fn await_block_params(&mut self, op: u8) -> Result<Vec<u8>, TransportError> {
        let deadline = Instant::now() + self.response_timeout;
        loop {
            while let Some(frame) = self.reader.next_frame() {
                let packet =
                    decode_packet(&frame).expect("FrameReader only yields frames that decode");
                if let Some(block) = packet.blocks.iter().find(|b| b.cmd == op) {
                    return Ok(block.params.clone());
                }
                self.queue_notify(frame);
            }
            if Instant::now() >= deadline {
                return Err(TransportError::Timeout);
            }
            let mut chunk = [0u8; MAX_FRAME];
            let socket = self.socket.as_mut().ok_or(TransportError::Disconnected)?;
            match socket.read(&mut chunk) {
                Ok(0) => {
                    self.socket = None;
                    return Err(TransportError::Disconnected);
                }
                Ok(n) => self.reader.push(&chunk[..n]),
                Err(TransportError::Timeout) => continue,
                Err(e) => {
                    self.socket = None;
                    return Err(e);
                }
            }
        }
    }
}

impl Transport for RfcommTransport {
    /// SPP devices do not advertise: discovery happens at the host pairing
    /// level (the earbuds are already paired for audio), so this transport
    /// surfaces no scan results. Callers connect by explicit address.
    fn scan(&mut self) -> Result<Vec<DiscoveredDevice>, TransportError> {
        Ok(Vec::new())
    }

    fn connect(&mut self, address: &str) -> Result<(), TransportError> {
        if self.socket.is_some() {
            self.disconnect()?;
        }
        let normalized = normalize_address(address);
        let socket = self.factory.open(&normalized, self.channel)?;
        self.socket = Some(socket);
        self.connected_address = Some(normalized);
        self.connected_model_known = false;
        self.reader = FrameReader::new();
        self.notify_queue.clear();
        self.subscribed = false;
        Ok(())
    }

    fn disconnect(&mut self) -> Result<(), TransportError> {
        self.socket = None;
        self.connected_address = None;
        self.connected_model_known = false;
        self.reader = FrameReader::new();
        self.notify_queue.clear();
        self.subscribed = false;
        Ok(())
    }

    fn read(&mut self, char_uuid: &str) -> Result<Vec<u8>, TransportError> {
        if self.socket.is_none() {
            return Err(TransportError::Disconnected);
        }
        let op = Self::read_opcode(char_uuid)
            .ok_or_else(|| TransportError::NotFound(char_uuid.to_string()))?;
        let request = encode_command(REQUEST_DATA, &[op])
            .map_err(|e| TransportError::InvalidArgument(format!("{e:?}")))?;
        // Defense in depth: even internally generated frames pass the central
        // policy. RequestData is authorized for read-only devices too.
        self.policy
            .authorize_frame(&request, self.experimental_opt_in)
            .map_err(TransportError::Denied)?;
        self.socket
            .as_mut()
            .expect("checked above")
            .write_all(&request)?;
        self.await_block_params(op)
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        if self.socket.is_none() {
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
        self.socket
            .as_mut()
            .expect("checked above")
            .write_all(bytes)
    }

    /// SPP has no GATT characteristics: direct (unframed) characteristic
    /// writes do not exist on this transport. EQ and key-function settings
    /// travel as framed opcodes through [`Transport::write`] instead.
    fn write_direct(&mut self, char_uuid: &str, _bytes: &[u8]) -> Result<(), TransportError> {
        if self.socket.is_none() {
            return Err(TransportError::Disconnected);
        }
        Err(TransportError::NotFound(char_uuid.to_string()))
    }

    /// Subscribe to unsolicited device frames. SPP has a single stream, so
    /// only the settings-notify equivalent is accepted; validated frames that
    /// arrive while answering reads are queued and exposed via
    /// [`RfcommTransport::pop_notify`].
    fn subscribe(&mut self, char_uuid: &str) -> Result<(), TransportError> {
        if self.socket.is_none() {
            return Err(TransportError::Disconnected);
        }
        if char_uuid.to_ascii_lowercase() == CHAR_SETTINGS_NOTIFY {
            self.subscribed = true;
            Ok(())
        } else {
            Err(TransportError::NotFound(char_uuid.to_string()))
        }
    }

    fn set_experimental_opt_in(&mut self, on: bool) {
        self.experimental_opt_in = on;
    }

    fn attest_model_known(&mut self) {
        if self.socket.is_some() {
            self.connected_model_known = true;
        }
    }

    /// The RFCOMM socket is the link: report its live state instead of the
    /// trait default (`false`), which would make a resident-session
    /// supervisor believe the link is permanently down and re-bootstrap in a
    /// loop (audit #68).
    fn is_connected(&mut self) -> Result<bool, TransportError> {
        Ok(self.socket.is_some())
    }

    fn session_address(&mut self) -> Option<String> {
        self.connected_address.clone()
    }
}

/* ------------------------------------------------------------------ */
/* Real raw-socket boundary (AF_BLUETOOTH / BTPROTO_RFCOMM)            */
/* ------------------------------------------------------------------ */

const AF_BLUETOOTH: libc::c_int = 31;
const BTPROTO_RFCOMM: libc::c_int = 3;

/// Kernel `struct sockaddr_rc` (`include/net/bluetooth/rfcomm.h`):
/// `{ sa_family_t rc_family; bdaddr_t rc_bdaddr; __u8 rc_channel; }`.
/// `libc` does not expose it, so it is reproduced with the stable ABI.
#[repr(C)]
struct SockaddrRc {
    family: u16,
    /// `bdaddr_t` stores octets in little-endian order: `b[0]` is the *last*
    /// octet of the printed address (same semantics as BlueZ `str2ba`).
    bdaddr: [u8; 6],
    channel: u8,
}

fn parse_bdaddr(address: &str) -> Result<[u8; 6], TransportError> {
    let octets: Vec<&str> = address.split(':').collect();
    if octets.len() != 6 {
        return Err(TransportError::InvalidArgument(format!(
            "invalid Bluetooth address: {address}"
        )));
    }
    let mut bytes = [0u8; 6];
    for (i, o) in octets.iter().enumerate() {
        bytes[i] = u8::from_str_radix(o, 16).map_err(|_| {
            TransportError::InvalidArgument(format!("invalid Bluetooth address: {address}"))
        })?;
    }
    Ok(bytes)
}

fn last_os_error() -> std::io::Error {
    std::io::Error::last_os_error()
}

/// `EAGAIN`/`EWOULDBLOCK` test that stays portable across Unixes (on Linux the
/// two constants are equal).
fn is_wouldblock(errno: Option<i32>) -> bool {
    errno == Some(libc::EAGAIN) || errno == Some(libc::EWOULDBLOCK)
}

/// Map a `connect(2)` failure to a structured transport error.
fn map_connect_error(err: std::io::Error, channel: u8) -> TransportError {
    match err.raw_os_error() {
        Some(libc::EHOSTDOWN)
        | Some(libc::EHOSTUNREACH)
        | Some(libc::ETIMEDOUT)
        | Some(libc::ENETUNREACH) => TransportError::DeviceOutOfRange,
        Some(libc::ENETDOWN) => TransportError::AdapterOff,
        Some(libc::EACCES) => TransportError::PermissionDenied,
        Some(libc::ECONNREFUSED) => TransportError::NotFound(format!(
            "RFCOMM channel {channel} refused (no service listening)"
        )),
        _ => TransportError::Bus(format!("RFCOMM connect failed: {err}")),
    }
}

/// Raw RFCOMM socket factory. Opens `AF_BLUETOOTH` / `SOCK_STREAM` /
/// `BTPROTO_RFCOMM` sockets; no root and no daemon reconfiguration required
/// for an already-paired device.
#[derive(Debug, Clone)]
pub struct RawRfcommSocketFactory {
    /// Per-read receive timeout (`SO_RCVTIMEO`); callers retry until their
    /// own deadline.
    recv_timeout: Duration,
}

impl RawRfcommSocketFactory {
    pub fn new(recv_timeout: Duration) -> Self {
        Self { recv_timeout }
    }
}

impl Default for RawRfcommSocketFactory {
    fn default() -> Self {
        Self::new(Duration::from_millis(200))
    }
}

impl RfcommSocketFactory for RawRfcommSocketFactory {
    fn open(&self, address: &str, channel: u8) -> Result<Box<dyn RfcommSocket>, TransportError> {
        let octets = parse_bdaddr(address)?;
        // bdaddr_t is little-endian: reverse the printed octets.
        let mut bdaddr = [0u8; 6];
        for (i, b) in octets.iter().rev().enumerate() {
            bdaddr[i] = *b;
        }
        let fd = unsafe { libc::socket(AF_BLUETOOTH, libc::SOCK_STREAM, BTPROTO_RFCOMM) };
        if fd < 0 {
            return Err(TransportError::Bus(format!(
                "socket(AF_BLUETOOTH) failed: {}",
                last_os_error()
            )));
        }
        let timeval = libc::timeval {
            tv_sec: self.recv_timeout.as_secs() as libc::time_t,
            tv_usec: self.recv_timeout.subsec_micros() as libc::suseconds_t,
        };
        let rc = unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &timeval as *const libc::timeval as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            let err = last_os_error();
            unsafe { libc::close(fd) };
            return Err(TransportError::Bus(format!("SO_RCVTIMEO failed: {err}")));
        }
        let addr = SockaddrRc {
            family: AF_BLUETOOTH as u16,
            bdaddr,
            channel,
        };
        let rc = unsafe {
            libc::connect(
                fd,
                &addr as *const SockaddrRc as *const libc::sockaddr,
                std::mem::size_of::<SockaddrRc>() as libc::socklen_t,
            )
        };
        if rc < 0 {
            let err = last_os_error();
            unsafe { libc::close(fd) };
            return Err(map_connect_error(err, channel));
        }
        Ok(Box::new(RawRfcommSocket { fd: Some(fd) }))
    }
}

/// A live RFCOMM socket, closed on drop.
pub struct RawRfcommSocket {
    fd: Option<libc::c_int>,
}

impl Drop for RawRfcommSocket {
    fn drop(&mut self) {
        if let Some(fd) = self.fd.take() {
            unsafe { libc::close(fd) };
        }
    }
}

impl RfcommSocket for RawRfcommSocket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        let fd = self.fd.ok_or(TransportError::Disconnected)?;
        let n = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
        if n >= 0 {
            return Ok(n as usize);
        }
        let err = last_os_error();
        if is_wouldblock(err.raw_os_error()) {
            return Err(TransportError::Timeout);
        }
        match err.raw_os_error() {
            Some(libc::ECONNRESET) | Some(libc::ENOTCONN) | Some(libc::EPIPE) => {
                self.fd = None;
                Err(TransportError::Disconnected)
            }
            _ => Err(TransportError::Bus(format!("RFCOMM read failed: {err}"))),
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        let fd = self.fd.ok_or(TransportError::Disconnected)?;
        let mut written = 0usize;
        while written < bytes.len() {
            let n = unsafe {
                libc::write(
                    fd,
                    bytes[written..].as_ptr() as *const libc::c_void,
                    bytes.len() - written,
                )
            };
            if n < 0 {
                let err = last_os_error();
                if is_wouldblock(err.raw_os_error()) {
                    return Err(TransportError::Timeout);
                }
                return match err.raw_os_error() {
                    Some(libc::ECONNRESET) | Some(libc::ENOTCONN) | Some(libc::EPIPE) => {
                        self.fd = None;
                        Err(TransportError::Disconnected)
                    }
                    _ => Err(TransportError::Bus(format!("RFCOMM write failed: {err}"))),
                };
            }
            written += n as usize;
        }
        Ok(())
    }
}

/* ------------------------------------------------------------------ */
/* Tests (scripted fake socket; no hardware, no D-Bus)                 */
/* ------------------------------------------------------------------ */

#[cfg(test)]
mod tests {
    use super::*;
    use qcy_protocol::packet::encode_blocks;
    use std::sync::{Arc, Mutex};

    /// One scripted socket event.
    #[derive(Debug, Clone)]
    enum IoEvent {
        /// Bytes returned by the next `read`.
        Data(Vec<u8>),
        /// Peer closed the connection (`Ok(0)`).
        Eof,
        /// Receive timeout elapsed with no data.
        Tick,
    }

    #[derive(Default)]
    struct FakePipe {
        rx: Mutex<VecDeque<IoEvent>>,
        tx: Mutex<Vec<Vec<u8>>>,
        opens: Mutex<Vec<(String, u8)>>,
    }

    struct FakeSocket {
        pipe: Arc<FakePipe>,
    }

    impl RfcommSocket for FakeSocket {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
            let mut rx = self.pipe.rx.lock().unwrap();
            match rx.pop_front() {
                Some(IoEvent::Data(data)) => {
                    let n = data.len().min(buf.len());
                    buf[..n].copy_from_slice(&data[..n]);
                    Ok(n)
                }
                Some(IoEvent::Eof) => Ok(0),
                Some(IoEvent::Tick) | None => Err(TransportError::Timeout),
            }
        }

        fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
            self.pipe.tx.lock().unwrap().push(bytes.to_vec());
            Ok(())
        }
    }

    struct FakeFactory {
        pipe: Arc<FakePipe>,
    }

    impl RfcommSocketFactory for FakeFactory {
        fn open(
            &self,
            address: &str,
            channel: u8,
        ) -> Result<Box<dyn RfcommSocket>, TransportError> {
            self.pipe
                .opens
                .lock()
                .unwrap()
                .push((address.to_string(), channel));
            Ok(Box::new(FakeSocket {
                pipe: Arc::clone(&self.pipe),
            }))
        }
    }

    fn transport() -> (RfcommTransport, Arc<FakePipe>) {
        let pipe = Arc::new(FakePipe::default());
        let t = RfcommTransport::new(
            Box::new(FakeFactory {
                pipe: Arc::clone(&pipe),
            }),
            WritePolicy::ht08(),
        )
        .with_response_timeout(Duration::from_millis(50));
        (t, pipe)
    }

    fn connected() -> (RfcommTransport, Arc<FakePipe>) {
        let (mut t, pipe) = transport();
        t.connect("84:AC:60:62:69:DA").unwrap();
        (t, pipe)
    }

    fn from_hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /* -------------------- FrameReader -------------------- */

    #[test]
    fn frame_reader_extracts_a_valid_frame() {
        let mut r = FrameReader::new();
        r.push(&from_hex("ff052f0352505e"));
        assert_eq!(r.next_frame(), Some(from_hex("ff052f0352505e")));
        assert_eq!(r.next_frame(), None);
    }

    #[test]
    fn frame_reader_skips_garbage_before_sof() {
        let mut r = FrameReader::new();
        r.push(&from_hex("0012abff052f0352505e"));
        assert_eq!(r.next_frame(), Some(from_hex("ff052f0352505e")));
    }

    #[test]
    fn frame_reader_reassembles_split_frames() {
        let mut r = FrameReader::new();
        r.push(&from_hex("ff05"));
        assert_eq!(r.next_frame(), None);
        r.push(&from_hex("2f0352"));
        assert_eq!(r.next_frame(), None);
        r.push(&from_hex("505e"));
        assert_eq!(r.next_frame(), Some(from_hex("ff052f0352505e")));
    }

    #[test]
    fn frame_reader_drops_invalid_candidates_and_rescans() {
        let mut r = FrameReader::new();
        // First candidate is complete (body_len=3) but truncated inside its
        // block; the reader must drop its SOF and find the valid frame.
        r.push(&from_hex("ff032f0500ff052f0352505e"));
        assert_eq!(r.next_frame(), Some(from_hex("ff052f0352505e")));
    }

    #[test]
    fn frame_reader_resyncs_after_a_stray_sof_with_implausible_length() {
        // Audit #68: a stray 0xFF followed by another 0xFF declares a 255-byte
        // body. Without the plausibility bound the reader stalled waiting for
        // 257 bytes and swallowed the valid frame behind the stray byte.
        let mut r = FrameReader::new();
        r.push(&from_hex("ffff052f0352505e"));
        assert_eq!(r.next_frame(), Some(from_hex("ff052f0352505e")));
        assert_eq!(r.next_frame(), None);
    }

    #[test]
    fn frame_reader_resyncs_when_only_the_stray_sof_has_arrived() {
        let mut r = FrameReader::new();
        r.push(&from_hex("ff"));
        assert_eq!(r.next_frame(), None);
        r.push(&from_hex("ff052f0352505e"));
        assert_eq!(r.next_frame(), Some(from_hex("ff052f0352505e")));
    }

    #[test]
    fn frame_reader_accepts_empty_body_frame() {
        let mut r = FrameReader::new();
        r.push(&from_hex("ff00"));
        assert_eq!(r.next_frame(), Some(from_hex("ff00")));
    }

    #[test]
    fn frame_reader_buffer_is_bounded_under_garbage_flood() {
        let mut r = FrameReader::new();
        let garbage = vec![0xAB; 8192];
        r.push(&garbage);
        r.push(&from_hex("ff052f0352505e"));
        assert_eq!(r.next_frame(), Some(from_hex("ff052f0352505e")));
    }

    /* -------------------- connect / discovery -------------------- */

    #[test]
    fn scan_surfaces_nothing_and_connect_records_address_and_channel() {
        let (mut t, pipe) = transport();
        assert!(t.scan().unwrap().is_empty());
        t.connect("84:ac:60:62:69:da").unwrap();
        assert_eq!(
            pipe.opens.lock().unwrap().as_slice(),
            &[("84:AC:60:62:69:DA".to_string(), DEFAULT_RFCOMM_CHANNEL)]
        );
        assert_eq!(t.connected_address(), Some("84:AC:60:62:69:DA"));
    }

    #[test]
    fn custom_channel_is_used() {
        let (t, pipe) = transport();
        let mut t = t.with_channel(3);
        t.connect("84:AC:60:62:69:DA").unwrap();
        assert_eq!(pipe.opens.lock().unwrap()[0].1, 3);
    }

    /* -------------------- reads -------------------- */

    #[test]
    fn battery_read_sends_request_data_and_returns_params() {
        let (mut t, pipe) = connected();
        pipe.rx
            .lock()
            .unwrap()
            .push_back(IoEvent::Data(from_hex("ff052f0352505e")));
        let bytes = t.read(CHAR_BATTERY).unwrap();
        assert_eq!(bytes, vec![0x52, 0x50, 0x5E]);
        assert_eq!(
            pipe.tx.lock().unwrap().as_slice(),
            &[from_hex("ff03fe012f")]
        );
    }

    #[test]
    fn version_read_maps_to_0x30() {
        let (mut t, pipe) = connected();
        pipe.rx
            .lock()
            .unwrap()
            .push_back(IoEvent::Data(from_hex("ff083006010402010402")));
        let bytes = t.read(CHAR_VERSION).unwrap();
        assert_eq!(bytes, vec![1, 4, 2, 1, 4, 2]);
        assert_eq!(
            pipe.tx.lock().unwrap().as_slice(),
            &[from_hex("ff03fe0130")]
        );
    }

    #[test]
    fn read_works_even_for_read_only_devices() {
        // RequestData is a read-back request: the policy allows it before any
        // model attestation, so status reads never require writability.
        let (mut t, pipe) = connected();
        assert!(!t.connected_model_known);
        pipe.rx
            .lock()
            .unwrap()
            .push_back(IoEvent::Data(from_hex("ff052f0352505e")));
        assert!(t.read(CHAR_BATTERY).is_ok());
    }

    #[test]
    fn read_times_out_when_the_device_is_silent() {
        let (mut t, pipe) = connected();
        // Receive-timeout ticks with no data, like a real SO_RCVTIMEO, until
        // the response window elapses.
        for _ in 0..3 {
            pipe.rx.lock().unwrap().push_back(IoEvent::Tick);
        }
        assert_eq!(t.read(CHAR_BATTERY), Err(TransportError::Timeout));
    }

    #[test]
    fn read_reports_disconnect_on_eof() {
        let (mut t, pipe) = connected();
        pipe.rx.lock().unwrap().push_back(IoEvent::Eof);
        assert_eq!(t.read(CHAR_BATTERY), Err(TransportError::Disconnected));
    }

    #[test]
    fn read_unknown_characteristic_is_not_found() {
        let (mut t, _pipe) = connected();
        assert!(matches!(
            t.read("0000dead-0000-1000-8000-00805f9b34fb"),
            Err(TransportError::NotFound(_))
        ));
    }

    #[test]
    fn read_requires_connection() {
        let (mut t, _pipe) = transport();
        assert_eq!(t.read(CHAR_BATTERY), Err(TransportError::Disconnected));
    }

    #[test]
    fn unsolicited_frames_are_queued_while_answering_a_read() {
        let (mut t, pipe) = connected();
        t.subscribe(CHAR_SETTINGS_NOTIFY).unwrap();
        // Device first pushes an unsolicited LowLatency frame, then answers.
        pipe.rx
            .lock()
            .unwrap()
            .push_back(IoEvent::Data(from_hex("ff03090101ff052f0352505e")));
        let bytes = t.read(CHAR_BATTERY).unwrap();
        assert_eq!(bytes, vec![0x52, 0x50, 0x5E]);
        assert_eq!(t.pop_notify(), Some(from_hex("ff03090101")));
        assert_eq!(t.pop_notify(), None);
    }

    #[test]
    fn unsolicited_frames_are_dropped_without_a_subscription() {
        // Audit #68: the `subscribed` flag is wired — mirroring GATT, notify
        // frames are only surfaced after subscribe().
        let (mut t, pipe) = connected();
        pipe.rx
            .lock()
            .unwrap()
            .push_back(IoEvent::Data(from_hex("ff03090101ff052f0352505e")));
        let bytes = t.read(CHAR_BATTERY).unwrap();
        assert_eq!(bytes, vec![0x52, 0x50, 0x5E]);
        assert_eq!(t.pop_notify(), None);
    }

    #[test]
    fn is_connected_tracks_the_socket_state() {
        // Audit #68: the trait default (always false) would make a
        // resident-session supervisor re-bootstrap in a loop.
        let (mut t, _pipe) = connected();
        assert_eq!(t.is_connected(), Ok(true));
        t.disconnect().unwrap();
        assert_eq!(t.is_connected(), Ok(false));
    }

    /* -------------------- writes / policy -------------------- */

    #[test]
    fn write_requires_connection() {
        let (mut t, _pipe) = transport();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert_eq!(t.write(&frame), Err(TransportError::Disconnected));
    }

    #[test]
    fn unknown_device_stays_read_only_over_spp() {
        // SPP exposes no advertised name: without attestation the device is
        // read-only and nothing reaches the wire.
        let (mut t, pipe) = connected();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice
            ))
        ));
        assert!(pipe.tx.lock().unwrap().is_empty());
    }

    #[test]
    fn attestation_lifts_read_only_and_the_frame_reaches_the_wire() {
        let (mut t, pipe) = connected();
        t.attest_model_known();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        t.write(&frame).unwrap();
        assert_eq!(pipe.tx.lock().unwrap().as_slice(), &[frame]);
    }

    #[test]
    fn falsified_0x0c_needs_opt_in_over_spp_like_everywhere_else() {
        // Audit #59: 0x0C NoiseCancelMode was demoted to write-experimental
        // (#53) after live HT08 validation showed the device ignores it. The
        // SPP path must enforce the same opt-in as GATT.
        let (mut t, pipe) = connected();
        t.attest_model_known();
        let frame = encode_command(0x0C, &[0x01]).unwrap();
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(
                crate::policy::Denial::ExperimentalWithoutOptIn(0x0C)
            ))
        ));
        assert!(pipe.tx.lock().unwrap().is_empty());
    }

    #[test]
    fn destructive_writes_are_denied_even_after_attestation() {
        let (mut t, pipe) = connected();
        t.attest_model_known();
        for op in [0x01u8, 0x02, 0x03] {
            let frame = encode_command(op, &[]).unwrap();
            assert!(matches!(
                t.write(&frame),
                Err(TransportError::Denied(
                    crate::policy::Denial::DestructiveOpcode(_)
                ))
            ));
        }
        assert!(pipe.tx.lock().unwrap().is_empty());
    }

    #[test]
    fn experimental_writes_require_opt_in_over_spp() {
        let (mut t, pipe) = connected();
        t.attest_model_known();
        let frame = encode_command(0x23, &[0x01]).unwrap();
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(
                crate::policy::Denial::ExperimentalWithoutOptIn(0x23)
            ))
        ));
        t.set_experimental_opt_in(true);
        t.write(&frame).unwrap();
        assert_eq!(pipe.tx.lock().unwrap().len(), 1);
    }

    #[test]
    fn attestation_is_cleared_by_disconnect_and_ignored_when_not_connected() {
        let (mut t, _pipe) = transport();
        t.attest_model_known(); // no-op without a connection
        t.connect("84:AC:60:62:69:DA").unwrap();
        let frame = encode_command(0x09, &[0x01]).unwrap();
        assert!(matches!(
            t.write(&frame),
            Err(TransportError::Denied(
                crate::policy::Denial::ReadOnlyDevice
            ))
        ));
    }

    /* -------------------- direct writes / subscribe -------------------- */

    #[test]
    fn write_direct_has_no_spp_equivalent() {
        let (mut t, _pipe) = connected();
        assert!(matches!(
            t.write_direct(crate::policy::CHAR_EQ_DIRECT, &[1, 2]),
            Err(TransportError::NotFound(_))
        ));
    }

    #[test]
    fn subscribe_accepts_only_the_settings_notify_equivalent() {
        let (mut t, _pipe) = connected();
        t.subscribe(CHAR_SETTINGS_NOTIFY).unwrap();
        assert!(matches!(
            t.subscribe(CHAR_BATTERY),
            Err(TransportError::NotFound(_))
        ));
    }

    /* -------------------- address parsing -------------------- */

    #[test]
    fn parse_bdaddr_accepts_colon_hex_and_rejects_garbage() {
        assert_eq!(
            parse_bdaddr("84:AC:60:62:69:DA").unwrap(),
            [0x84, 0xAC, 0x60, 0x62, 0x69, 0xDA]
        );
        assert!(matches!(
            parse_bdaddr("84:AC:60:62:69"),
            Err(TransportError::InvalidArgument(_))
        ));
        assert!(matches!(
            parse_bdaddr("84:AC:60:62:69:ZZ"),
            Err(TransportError::InvalidArgument(_))
        ));
    }

    #[test]
    fn multi_block_answer_returns_the_requested_block_params() {
        let (mut t, pipe) = connected();
        let answer = encode_blocks(&[
            qcy_protocol::packet::CommandBlock {
                cmd: 0x09,
                params: vec![0x01],
            },
            qcy_protocol::packet::CommandBlock {
                cmd: 0x2F,
                params: vec![0x52, 0x50, 0x5E],
            },
        ])
        .unwrap();
        pipe.rx.lock().unwrap().push_back(IoEvent::Data(answer));
        assert_eq!(t.read(CHAR_BATTERY).unwrap(), vec![0x52, 0x50, 0x5E]);
    }
}
