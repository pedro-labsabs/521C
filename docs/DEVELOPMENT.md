# Development

## Prerequisites

- Node.js 22+ and npm
- Stable Rust toolchain for `native/`
- Linux is the primary runtime target for native Bluetooth integration

## Setup

Node.js 22+ and npm are required. Install locked dependencies for a clean,
reproducible checkout:

```bash
npm ci
npm run dev
```

The dev server binds to loopback (`127.0.0.1`) by default — 521C is local-first
and does not expose the dev server to the LAN. If you explicitly need to reach it
from another machine on your network, use the opt-in command `npm run dev:lan`.

Use `npm install` only when intentionally changing the dependency graph; it
updates `package-lock.json`, which must be committed together with the
`package.json` change. The committed lockfile is the source of truth for CI
and clean validation environments.

For the Rust workspace:

```bash
cd native
cargo test --workspace
```

The CLI crate is named `five21cctl` because Cargo package names cannot start
with a digit; the built binary keeps the contracted name `521cctl`.

## Native transport & CLI (issue #7)

The `native/crates/qcy-transport` crate provides the transport layer used by the
CLI and (later) the Slint GUI. It exposes a single `Transport` trait with two
backends: a deterministic `MockTransport` and a `BlueZTransport` that drives the
system BlueZ stack over D-Bus GATT. All outbound writes pass through the central
write-authorization policy before reaching the wire.

The CLI (`521cctl`) defaults to the mock transport so it runs anywhere with no
hardware. Pass `--bluez` to operate a real, explicitly selected device:

```bash
cd native
cargo run -p five21cctl --bin 521cctl -- scan                 # mock
cargo run -p five21cctl --bin 521cctl -- --bluez scan         # real discovery
cargo run -p five21cctl --bin 521cctl -- --bluez status       # real readout
cargo run -p five21cctl --bin 521cctl -- --bluez anc transparency
```

Useful flags: `--adapter hci0` selects the adapter, `--device <addr>` pins the
target. Discovery only surfaces candidate QCY devices; a device whose model is not
proven from its name is reported and treated as read-only. The live D-Bus boundary
is compiled behind the `bluez` Cargo feature (default on); the transport trait,
object mapping, policy and mock always build and are unit-tested without hardware.

## Desktop app (issue #8)

The native desktop application is `native/crates/521c-desktop` (crate
`five21c-desktop`, installed binary `521c`): a Slint GUI over the `qcy-app`
core. Architecture decision and boundary rules: `docs/DESKTOP_ARCHITECTURE.md`.

```bash
cd native
cargo run -p five21c-desktop --bin 521c             # BlueZ transport (default)
cargo run -p five21c-desktop --bin 521c -- --mock   # clearly labelled mock backend
cargo run -p five21c-desktop --bin 521c -- --self-test   # launch check, exits 0
```

The app falls back to the mock transport (visibly) when the BlueZ system bus is
unavailable. Configuration persists at `~/.config/521c/config.json` (XDG; the
same validated JSON contract as the browser schema, issue #11).

Runtime dependencies on Linux Mint-class systems: BlueZ + system D-Bus for real
hardware, a D-Bus session bus for MPRIS, PipeWire user config for system EQ, and
fontconfig/libxkbcommon plus an X11/Wayland session for the window. Normal use
never requires root.

### AppImage

Build the self-contained artifact (release build + AppDir + appimagetool):

```bash
scripts/package-appimage.sh        # needs appimagetool; see below
ls native/dist/521C-*.AppImage
```

`appimagetool` is official AppImage project release tooling and is not vendored
in the repository. Get it from <https://github.com/AppImage/appimagetool/releases>
and either put it on `PATH` or point `$APPIMAGETOOL` at it. CI builds the same
artifact on every push (job `desktop-package`).

Install/uninstall (user-local, no root):

```bash
# install: keep the AppImage and register a menu entry
mkdir -p ~/Applications ~/.local/share/applications
cp native/dist/521C-0.1.0-x86_64.AppImage ~/Applications/
cp packaging/linux/521c.desktop ~/.local/share/applications/
mkdir -p ~/.local/share/icons/hicolor/256x256/apps
cp native/crates/521c-desktop/assets/icons/521c_256.png \
   ~/.local/share/icons/hicolor/256x256/apps/521c.png
update-desktop-database ~/.local/share/applications || true

# uninstall
rm -f ~/Applications/521C-*.AppImage \
      ~/.local/share/applications/521c.desktop \
      ~/.local/share/icons/hicolor/*/apps/521c.png
update-desktop-database ~/.local/share/applications || true
# user config (optional): rm -rf ~/.config/521c
```

Alternatively, run the AppImage directly without installing:
`./521C-0.1.0-x86_64.AppImage`.

## Web Bluetooth (development transport)

The web app defaults to the mock transport, which is visibly labelled
"Mock preview" in the header. To exercise a real device during development:

1. Use a Chromium browser over a secure context (HTTPS or `localhost`).
2. Click **Connect real device** — this is an explicit user gesture, which Web
   Bluetooth requires for `requestDevice`.
3. Choose the QCY device in the browser picker.

Real sessions start with unknown telemetry (`--` battery, blank firmware) and
populate proven fields from the initial state sync and notifications; they never
borrow mock battery/firmware values. Real-device writes still pass through the
central write-authorization policy. Web Bluetooth is a development path only —
the primary Linux transport is native BlueZ (issue #7).

## Host services (issue #13)

`native/crates/qcy-host` provides Linux host functionality that is separate from the
QCY vendor protocol and never written to the earbuds. It is exposed through `521cctl`:

```bash
521cctl media status                 # MPRIS now-playing
521cctl media play|pause|next|prev   # MPRIS control
521cctl codec                        # host codec/sample-rate (unknown if unavailable)
521cctl system-eq on [10 gains...]   # create the user PipeWire EQ artifact
521cctl system-eq off                # remove it
521cctl system-eq status
```

Linux Mint runtime dependencies for these features:

- **MPRIS** — a D-Bus session bus plus at least one running MPRIS-capable player
  (`org.mpris.MediaPlayer2.*`). With none present, commands report "no MPRIS player".
- **Codec/sample-rate** — read passively from BlueZ `MediaTransport1` objects on the
  system bus (codec assigned number, configuration blob and profile UUID, per BlueZ
  `doc/org.bluez.MediaTransport.rst` and `profiles/audio/a2dp-codecs.h`). The active
  transport is preferred; sample rate is parsed for SBC/MPEG/AAC/LDAC layouts. Fields
  that cannot be sourced are reported as `unknown` (never invented). No transport is
  acquired or modified.
- **System EQ** — PipeWire with user config under `~/.config/pipewire/pipewire.conf.d/`.
  521C creates/removes only its own `521c-system-eq.conf`; `system-eq status` reads the
  artifact from disk, so it is correct across CLI invocations. Applying requires
  PipeWire to reload (typically a session restart). No system-wide config is touched.
- **Auto Game Mode** — the chosen host signal is MPRIS player presence: players
  appearing/disappearing as `org.mpris.MediaPlayer2.*` bus names are delivered by the
  session bus as `NameOwnerChanged` signals (a genuine subscription, no polling). The
  candidate name matched against the keyword allowlist is the bus-name suffix (e.g.
  `vlc`, `steam`). The controller debounces transitions; applications without an MPRIS
  name cannot trigger game mode. Device writes happen only through the central write
  policy once the desktop application (#8) wires the controller.

Missing services are handled gracefully. The live D-Bus integration is behind the `dbus`
Cargo feature (default on); the traits, rule engine and lifecycle logic always build and
are unit-tested against fakes, so no live service is needed to run the tests.

## Validation ladder

Run the narrowest relevant test while iterating, then run the full gate before a pull request is considered ready. From a clean checkout, install with `npm ci` first so the locked dependency graph is used:

```bash
npm ci
npm test
npm run typecheck
npm run lint
npm run build
npm run audit:network
cd native
cargo test --workspace
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

`npm run audit:network` fails if the built application or the default shell
contains implicit third-party runtime URLs (issue #12); see
`docs/SECURITY_MODEL.md` for the network-behavior contract.

GitHub Actions (`.github/workflows/ci.yml`) runs this same ladder on Node 22 and stable Rust for every push to `main` and every pull request.

Or, with `just` installed:

```bash
just check
```

## Testing

`npm test` runs [vitest](https://vitest.dev) over `src/**/*.test.ts`. Tests are
colocated with the production code they exercise and import the real modules —
there are no re-implemented "shadow" helpers. A standalone `vitest.config.ts`
keeps protocol/state tests in plain Node without loading the TanStack Start
build plugins.

Byte-level protocol behavior is pinned by a **shared conformance corpus** at
`conformance/protocol_vectors.json`, consumed by both implementations:

- TypeScript: `src/lib/qcy/protocol/conformance.test.ts`
- Rust: `native/crates/qcy-protocol/tests/conformance.rs`

A cross-language semantic divergence covered by a vector fails at least one
gate. See `conformance/README.md` for the schema, provenance rules, and how to
add a vector.

## Protocol work

Protocol changes need evidence. Prefer captured packets, reproducible traces, public documentation, or hardware verification. Record uncertainty explicitly instead of turning assumptions into supported capabilities.

When adding or changing a write path:

1. Add or update a conformance vector in `conformance/protocol_vectors.json` first (fixture before codec/write change).
2. Validate input length and range.
3. Keep raw GATT bytes below the UI/state boundary.
4. Update `docs/PROTOCOL.md` and `docs/SUPPORTED_DEVICES.md` if capability semantics changed.
5. Never make destructive commands reachable from unattended automation.

## Generated files

`src/routeTree.gen.ts` is generated by TanStack Router. Do not hand-edit it unless the generator itself is the subject of the change.
