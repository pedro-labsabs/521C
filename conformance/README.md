# Protocol conformance vectors

`protocol_vectors.json` is the single shared corpus of byte-level protocol
vectors consumed by **both** protocol implementations:

- TypeScript: `src/lib/qcy/protocol/conformance.test.ts` (vitest)
- Rust: `native/crates/qcy-protocol/tests/conformance.rs`

Any semantic divergence between the two implementations that is covered by a
vector fails at least one repository gate (`npm test` or `cargo test`).

## Provenance

Every vector is derived from `docs/PROTOCOL.md` (independent reverse-engineering
notes) and the pre-existing in-repo test fixtures. **No guessed bytes.** Each
entry is one of:

- a documented frame (e.g. `RequestData(0xFE)` asking `Battery(0x2F)`);
- a boundary construction of the documented framing rules (empty block, empty
  body, truncated header, length mismatch, bad SOF);
- a rejection case (wrong company id, short buffer).

Larger boundary cases that are awkward to express as literal hex (decode buffer
> `maxPacket`, encode param/body overflow) are constructed programmatically in
each language's test rather than stored here, so this file stays readable and
free of generator logic.

## Schema

```jsonc
{
  "version": 1,
  "frame": { "sof": "0xff", "maxPacket": 512, "layout": "..." },
  "decode": [        // run through decode_packet / decodePacket
    { "name": "...", "hex": "ff03fe012f",
      "expect": { "ok": true,  "blocks": [ { "cmd": 254, "paramsHex": "2f" } ] } },
    { "name": "...", "hex": "000100",
      "expect": { "ok": false, "error": "bad-sof" } }
  ],
  "encode": [        // run through encode_blocks / encodeBlocks
    { "name": "...", "blocks": [ { "cmd": 254, "paramsHex": "2f" } ],
      "expectHex": "ff03fe012f" }
  ],
  "advertisement": [ // parse_manufacturer_data / parseManufacturerData
    { "name": "...", "companyId": 21020, "dataHex": "...",
      "expect": { "vendorId": 4660, "battery": {...}, "controlMac": "...", "otherMac": "..." } }
  ],
  "battery": [       // BatteryState::decode / parseBatteryBytes
    { "name": "...", "hex": "52505e", "expect": { "left": {...}, "right": {...}, "case": {...} } }
  ],
  "firmware": {      // TypeScript-only; Rust core has no firmware parser
    "consumers": ["typescript"], "vectors": [ ... ]
  }
}
```

### Error identifiers

Decode error names are canonical kebab-case and mapped to each language's enum:

| vector `error` | TS `DecodeError.kind` | Rust `DecodeError` |
| --- | --- | --- |
| `too-short` | `too-short` | `TooShort` |
| `bad-sof` | `bad-sof` | `BadSof` |
| `length-mismatch` | `length-mismatch` | `LengthMismatch` |
| `truncated-block` | `truncated-block` | `TruncatedBlock` |
| `oversize` | `oversize` | `Oversize` |

### Shared vs language-specific fields

Consumers compare only the overlapping semantic fields. For example the Rust
`Advertisement` has no `colorIndex`/`rawLength`, so advertisement vectors assert
`vendorId`, `battery`, `controlMac`, and `otherMac` only. The `firmware` section
is consumed by TypeScript alone.

## Adding a vector

1. Add or update the vector here **before** enabling any new write path
   (repository contract: fixture first, then the codec/write change).
2. Keep bytes evidence-backed; do not add guessed payloads to increase coverage.
3. Both language tests pick the vector up automatically on next run.
