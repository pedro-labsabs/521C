# Contributing

1. Do not invent UUIDs, opcodes, or vendor IDs.
2. Validate every BLE frame (SOF, length, bounds).
3. Mark capabilities honestly: supported / experimental / unknown / needs-research / unsupported.
4. Never send `0x01` / `0x02` / `0x03` from automations.
5. Never flash firmware unless format, checksums, and recovery are proven.
6. Add a parser fixture before a write path.
7. Keep HT08-specific behavior inside `Ht08Profile`, not `if model == "HT08"` in the UI.

```bash
npm test
cd native && cargo test --workspace && cargo clippy --all-targets -- -D warnings
```
