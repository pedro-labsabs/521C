//! Shared SPP/RFCOMM conformance tests (issue #50).
//!
//! Consumes the `spp` section of `conformance/protocol_vectors.json` and
//! drives a real [`RfcommTransport`] against a scripted in-memory socket, so
//! the characteristic -> RequestData mapping stays pinned by the shared corpus.
//! Every byte in the corpus is composed from vectors already documented there;
//! HT08 on-wire confirmation is tracked separately in the evidence ledger.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use qcy_transport::policy::WritePolicy;
use qcy_transport::rfcomm::{
    RfcommSocket, RfcommSocketFactory, RfcommTransport, DEFAULT_RFCOMM_CHANNEL,
};
use qcy_transport::{Transport, TransportError};
use serde::Deserialize;

const VECTORS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../conformance/protocol_vectors.json"
));

fn from_hex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
        .collect()
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[derive(Deserialize)]
struct Corpus {
    spp: SppSection,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SppSection {
    default_channel: u8,
    reads: Vec<SppReadVector>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SppReadVector {
    name: String,
    char_uuid: String,
    request_hex: String,
    response_hex: String,
    expect_hex: String,
}

/* ---------------- scripted socket (self-contained) ---------------- */

#[derive(Default)]
struct Pipe {
    rx: Mutex<VecDeque<Vec<u8>>>,
    tx: Mutex<Vec<Vec<u8>>>,
}

struct PipeSocket {
    pipe: Arc<Pipe>,
}

impl RfcommSocket for PipeSocket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, TransportError> {
        match self.pipe.rx.lock().unwrap().pop_front() {
            Some(data) => {
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                Ok(n)
            }
            None => Err(TransportError::Timeout),
        }
    }

    fn write_all(&mut self, bytes: &[u8]) -> Result<(), TransportError> {
        self.pipe.tx.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }
}

struct PipeFactory {
    pipe: Arc<Pipe>,
}

impl RfcommSocketFactory for PipeFactory {
    fn open(&self, _address: &str, _channel: u8) -> Result<Box<dyn RfcommSocket>, TransportError> {
        Ok(Box::new(PipeSocket {
            pipe: Arc::clone(&self.pipe),
        }))
    }
}

fn corpus() -> Corpus {
    serde_json::from_str(VECTORS).expect("corpus parses")
}

#[test]
fn default_channel_matches_the_corpus() {
    assert_eq!(corpus().spp.default_channel, DEFAULT_RFCOMM_CHANNEL);
}

#[test]
fn spp_read_vectors_pin_the_request_response_mapping() {
    for v in corpus().spp.reads {
        let pipe = Arc::new(Pipe::default());
        let mut t = RfcommTransport::new(
            Box::new(PipeFactory {
                pipe: Arc::clone(&pipe),
            }),
            WritePolicy::ht08(),
        )
        .with_response_timeout(Duration::from_millis(50));
        t.connect("84:AC:60:62:69:DA").unwrap();

        pipe.rx.lock().unwrap().push_back(from_hex(&v.response_hex));
        let got = t
            .read(&v.char_uuid)
            .unwrap_or_else(|e| panic!("{}: read failed: {e}", v.name));

        assert_eq!(
            to_hex(&got),
            v.expect_hex,
            "{}: read payload diverges from the corpus",
            v.name
        );
        let tx = pipe.tx.lock().unwrap();
        assert_eq!(tx.len(), 1, "{}: exactly one request frame sent", v.name);
        assert_eq!(
            to_hex(&tx[0]),
            v.request_hex,
            "{}: request frame diverges from the corpus",
            v.name
        );
    }
}
