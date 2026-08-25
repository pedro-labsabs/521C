## Issue / objective
<!-- Link the issue(s) or state the bounded objective and acceptance criteria. -->

## Summary

## Scope and architecture
<!-- What changed? Which layer owns the behavior? Note any dependency/interface decision. -->

## Evidence / protocol basis
<!-- Required when changing protocol semantics, UUID/opcode trust, device mappings or capabilities. Distinguish observed/proven facts from research leads. -->

## Safety / host impact
<!-- Note BLE writes, write-policy changes, destructive-command reachability, permissions, system packages/services, networking, privacy or firmware implications. -->

## Hardware verification
<!-- State exactly what was exercised on real HT08 hardware. Write "not hardware-verified" where appropriate; do not imply mock coverage proves hardware behavior. -->

## Tests added / changed

## Validation
- [ ] Clean dependency/setup path used where applicable (`npm ci` after lockfile exists)
- [ ] `npm test`
- [ ] `npm run typecheck`
- [ ] `npm run lint`
- [ ] `npm run build`
- [ ] `cd native && cargo test --workspace`
- [ ] `cd native && cargo fmt --check`
- [ ] `cd native && cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Relevant package/native release check, if this PR touches desktop/release code
- [ ] `git diff`/changed files inspected for secrets, generated noise and unrelated changes

## Acceptance criteria review
<!-- Map the issue's criteria to concrete implementation/tests/evidence. -->

## Remaining uncertainty / follow-up
<!-- Known limitations must be explicit. Do not use "should work" in place of evidence. -->
