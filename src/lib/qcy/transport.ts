import {
  CHAR,
  Cmd,
  SERVICE,
  decodePacket,
  encodeBatteryBytes,
  encodeCommand,
  encodeKeyFunctionDirect,
  encodeManufacturerData,
  parseAncScene,
  parseBatteryBytes,
  parseEqV2,
  parseFirmwareBytes,
  parseKeyFunctionBytes,
  parseManufacturerData,
  parseWear,
  QCY_COMPANY_ID,
  cmdName,
  enableByte,
  parseEnable,
  type Advertisement,
  type AncScene,
  type BatteryState,
  type CommandBlock,
  type EqPreset,
  type KeyBinding,
  type WearSettings,
} from "./protocol";
import { DEVICE_EQ_PRESETS } from "./eq-presets";
import { HT08_PROFILE, identifyProfile, type QcyDeviceProfile } from "./device/catalog";
import { FunId, KeyId } from "./protocol/types";
import {
  authorizeDirectWrite,
  authorizeFrameWrite,
  DEFAULT_OPT_IN,
  WriteDeniedError,
  type SessionOptIn,
} from "./policy";

export type TransportKind = "mock" | "web-bluetooth";

export type DiscoveredDevice = {
  id: string;
  name: string;
  address: string;
  rssi: number;
  advertisement: Advertisement | null;
  profile: QcyDeviceProfile;
  kind: TransportKind;
};

export type AudioGraph = {
  codec: "SBC" | "AAC" | "LDAC" | "unknown";
  sampleRate: number | null;
  channels: number | null;
  profile: "a2dp-sink" | "hfp" | "unknown";
  bitrateKbps: number | null;
  sink: string;
  source: string;
};

export type MediaState = {
  playing: boolean;
  title: string;
  artist: string;
  player: string;
  volume: number;
};

export type DeviceLiveState = {
  connected: boolean;
  connecting: boolean;
  name: string;
  address: string;
  profile: QcyDeviceProfile;
  battery: BatteryState;
  firmware: { left: string; right?: string };
  rssi: number;
  noiseMode: number;
  ancScene: AncScene;
  adaptive: boolean;
  gameMode: boolean;
  sleepMode: boolean;
  spatial: boolean;
  inEar: boolean;
  wornLeft: boolean;
  wornRight: boolean;
  wear: WearSettings;
  eq: EqPreset;
  eqName: string;
  bindings: KeyBinding[];
  ldacRequested: boolean;
  soundBalance: number;
  audio: AudioGraph;
  media: MediaState;
  lastSeen: { at: string; host: string; rssi: number } | null;
  /**
   * False until real telemetry has been observed (issue #2). Real sessions start
   * with unknown battery/firmware rather than mock values; the UI renders "--" and
   * battery notifications stay suppressed while this is false.
   */
  telemetryKnown: boolean;
};

export type PacketLogEntry = {
  id: number;
  at: number;
  dir: "tx" | "rx";
  hex: string;
  summary: string;
  cmd: number;
};

export type TransportEvents = {
  onState: (state: DeviceLiveState) => void;
  onLog: (entry: PacketLogEntry) => void;
  onDisconnected: (reason: string) => void;
};

let logSeq = 1;

function defaultBindings(): KeyBinding[] {
  return [
    { keyId: KeyId.MusicLeftSingle, funId: FunId.PlayPause },
    { keyId: KeyId.MusicRightSingle, funId: FunId.PlayPause },
    { keyId: KeyId.MusicLeftDouble, funId: FunId.Previous },
    { keyId: KeyId.MusicRightDouble, funId: FunId.Next },
    { keyId: KeyId.MusicLeftTriple, funId: FunId.VolumeDown },
    { keyId: KeyId.MusicRightTriple, funId: FunId.VolumeUp },
    { keyId: KeyId.MusicLeftHold, funId: FunId.GameMode },
    { keyId: KeyId.MusicRightHold, funId: FunId.None },
    { keyId: KeyId.CallLeftSingle, funId: FunId.AnswerCall },
    { keyId: KeyId.CallRightSingle, funId: FunId.AnswerCall },
    { keyId: KeyId.CallLeftDouble, funId: FunId.RejectCall },
    { keyId: KeyId.CallRightDouble, funId: FunId.RejectCall },
    { keyId: KeyId.CallLeftHold, funId: FunId.None },
    { keyId: KeyId.CallRightHold, funId: FunId.None },
  ];
}

export function createInitialState(partial?: Partial<DeviceLiveState>): DeviceLiveState {
  const eq = DEVICE_EQ_PRESETS[0]!.preset;
  return {
    connected: false,
    connecting: false,
    name: HT08_PROFILE.title,
    address: "F8:5C:7D:12:08:08",
    profile: HT08_PROFILE,
    battery: {
      left: { level: 82, charging: false },
      right: { level: 80, charging: false },
      case: { level: 94, charging: false },
    },
    firmware: { left: "1.4.2", right: "1.4.2" },
    rssi: -48,
    noiseMode: 0x01,
    ancScene: { mode: 0x02, subScene: 0x02, noiseValue: 80 },
    adaptive: false,
    gameMode: false,
    sleepMode: false,
    spatial: false,
    inEar: true,
    wornLeft: true,
    wornRight: true,
    wear: { enabled: true, musicIndex: 1, ancIndex: 0, toneEnable: true },
    eq,
    eqName: "Flat",
    bindings: defaultBindings(),
    ldacRequested: true,
    soundBalance: 0x32,
    audio: {
      codec: "LDAC",
      sampleRate: 96000,
      channels: 2,
      profile: "a2dp-sink",
      bitrateKbps: null,
      sink: "bluez_output.HT08",
      source: "bluez_input.HT08",
    },
    media: {
      playing: true,
      title: "Night Drive",
      artist: "Local player",
      player: "mpv",
      volume: 72,
    },
    lastSeen: null,
    telemetryKnown: true,
    ...partial,
  };
}

/**
 * Initial state for a real-device session (issue #2). Telemetry starts unknown rather
 * than borrowing mock battery/firmware/settings values; the initial state sync and
 * subsequent notifications populate proven fields and flip `telemetryKnown`.
 */
export function createRealInitialState(partial?: Partial<DeviceLiveState>): DeviceLiveState {
  return createInitialState({
    name: "QCY device",
    address: "",
    battery: {
      left: { level: 0, charging: false },
      right: { level: 0, charging: false },
      case: { level: 0, charging: false },
    },
    firmware: { left: "" },
    rssi: 0,
    eqName: "",
    audio: {
      codec: "unknown",
      sampleRate: null,
      channels: 2,
      profile: "unknown",
      bitrateKbps: null,
      sink: "system",
      source: "system",
    },
    media: { playing: false, title: "", artist: "", player: "", volume: 0 },
    telemetryKnown: false,
    ...partial,
  });
}

export interface QcyTransport {
  kind: TransportKind;
  scan(): Promise<DiscoveredDevice[]>;
  connect(id: string, events: TransportEvents): Promise<void>;
  disconnect(): Promise<void>;
  /**
   * Framed write to the command characteristic. Must pass the central
   * write-authorization policy; denied writes reject with WriteDeniedError.
   */
  write(bytes: Uint8Array): Promise<void>;
  /**
   * Unframed write to a specific allowlisted characteristic. Must pass the
   * central write-authorization policy; denied writes reject with WriteDeniedError.
   */
  writeDirect(charUuid: string, bytes: Uint8Array): Promise<void>;
  read(charUuid: string): Promise<Uint8Array>;
  /**
   * Session-scoped opt-in for experimental writes. Not persisted across
   * sessions; transports reset it to the default on construction.
   */
  setExperimentalOptIn(on: boolean): void;
}

function respond(cmd: number, params: ArrayLike<number>): Uint8Array {
  return encodeCommand(cmd, params);
}

function eqToParams(eq: EqPreset): number[] {
  const params: number[] = [eq.index & 0xff];
  const mg = Math.round(eq.masterGainDb * 100);
  const mgu = mg < 0 ? mg + 0x10000 : mg;
  params.push(mgu & 0xff, (mgu >> 8) & 0xff);
  for (const band of eq.bands) {
    const g = Math.round(Math.max(-12.7, Math.min(12.7, band.gainDb)) * 100);
    const gu = g < 0 ? g + 0x10000 : g;
    const q = Math.round(band.q * 100);
    params.push(
      band.freqHz & 0xff,
      (band.freqHz >> 8) & 0xff,
      gu & 0xff,
      (gu >> 8) & 0xff,
      q & 0xff,
      (q >> 8) & 0xff,
      (band.bandType ?? 0) & 0xff,
    );
  }
  return params;
}

function delay(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

export class MockTransport implements QcyTransport {
  kind: TransportKind = "mock";
  private state = createInitialState();
  private events: TransportEvents | null = null;
  private rssiTimer: ReturnType<typeof setInterval> | null = null;
  private smoothRssi = -48;
  private optIn: SessionOptIn = { ...DEFAULT_OPT_IN };

  setExperimentalOptIn(on: boolean): void {
    this.optIn = { ...this.optIn, experimental: on };
  }

  private advBytes(): Uint8Array {
    return encodeManufacturerData({
      vendorId: 0,
      battery: this.state.battery,
      controlMac: this.state.address,
      otherMac: this.state.address,
    });
  }

  scan(): Promise<DiscoveredDevice[]> {
    const adv = parseManufacturerData(QCY_COMPANY_ID, this.advBytes());
    return Promise.resolve([
      {
        id: "mock-ht08",
        name: HT08_PROFILE.title,
        address: this.state.address,
        rssi: this.state.rssi,
        advertisement: adv,
        profile: HT08_PROFILE,
        kind: "mock",
      },
    ]);
  }

  async connect(_id: string, events: TransportEvents): Promise<void> {
    this.events = events;
    this.state = { ...this.state, connecting: true, connected: false };
    events.onState(this.state);
    await delay(40);
    this.state = {
      ...this.state,
      connecting: false,
      connected: true,
      lastSeen: {
        at: new Date().toISOString(),
        host: "this-computer",
        rssi: this.state.rssi,
      },
    };
    events.onState(this.state);
    this.emitRx(respond(Cmd.Battery, encodeBatteryBytes(this.state.battery)));
    const [maj, min, pat] = this.state.firmware.left.split(".").map((n) => Number(n) || 0);
    this.emitRx(respond(Cmd.Version, [maj, min, pat, maj, min, pat]));
    this.startRssi();
  }

  async disconnect(): Promise<void> {
    this.stopRssi();
    this.state = { ...this.state, connected: false, connecting: false };
    const ev = this.events;
    this.events = null;
    ev?.onDisconnected("disconnected");
  }

  async write(bytes: Uint8Array): Promise<void> {
    const auth = authorizeFrameWrite(this.state.profile, this.optIn, bytes);
    if (!auth.ok) throw new WriteDeniedError(auth.denial);
    const decoded = decodePacket(bytes);
    const summary = decoded.ok
      ? decoded.packet.blocks.map((b) => cmdName(b.cmd)).join(",")
      : decoded.error.kind;
    this.emitLog("tx", bytes, summary, decoded.ok ? (decoded.packet.blocks[0]?.cmd ?? 0) : 0);
    if (!decoded.ok) return;
    for (const block of decoded.packet.blocks) {
      this.handleBlock(block);
    }
  }

  async writeDirect(charUuid: string, bytes: Uint8Array): Promise<void> {
    const auth = authorizeDirectWrite(this.state.profile, this.optIn, charUuid, bytes);
    if (!auth.ok) throw new WriteDeniedError(auth.denial);
    this.emitLog("tx", bytes, `direct ${charUuid.slice(4, 8)}`, 0);
    if (charUuid.toLowerCase() === CHAR.keyFunctionV2) {
      this.state = { ...this.state, bindings: parseKeyFunctionBytes(bytes) };
      this.push();
    }
  }

  async read(charUuid: string): Promise<Uint8Array> {
    if (charUuid.toLowerCase() === CHAR.battery) {
      return encodeBatteryBytes(this.state.battery);
    }
    if (charUuid.toLowerCase() === CHAR.version) {
      const [a, b, c] = this.state.firmware.left.split(".").map((n) => Number(n) || 0);
      return Uint8Array.from([a, b, c]);
    }
    if (charUuid.toLowerCase() === CHAR.keyFunctionV2) {
      return encodeKeyFunctionDirect(this.state.bindings);
    }
    return new Uint8Array();
  }

  applyLocalEqName(name: string) {
    this.state = { ...this.state, eqName: name };
    this.push();
  }

  setWorn(left: boolean, right: boolean) {
    this.state = { ...this.state, wornLeft: left, wornRight: right };
    if (this.state.wear.enabled && !left && !right && this.state.wear.musicIndex === 1) {
      this.state = { ...this.state, media: { ...this.state.media, playing: false } };
    }
    this.push();
  }

  private handleBlock(block: CommandBlock) {
    const p = block.params;
    switch (block.cmd) {
      case Cmd.RequestData: {
        const want = p[0] ?? 0;
        this.handleBlock({ cmd: want, params: new Uint8Array() });
        return;
      }
      case Cmd.Battery:
        this.emitRx(respond(Cmd.Battery, encodeBatteryBytes(this.state.battery)));
        return;
      case Cmd.Version: {
        const [a, b, c] = this.state.firmware.left.split(".").map((n) => Number(n) || 0);
        this.emitRx(respond(Cmd.Version, [a, b, c, a, b, c]));
        return;
      }
      case Cmd.NoiseCancelMode: {
        if (p.length) this.state = { ...this.state, noiseMode: p[0]!, adaptive: false };
        this.emitRx(respond(Cmd.NoiseCancelMode, [this.state.noiseMode]));
        this.push();
        return;
      }
      case Cmd.AncSetting: {
        if (p.length >= 3) {
          this.state = {
            ...this.state,
            ancScene: { mode: p[0]!, subScene: p[1]!, noiseValue: p[2]! },
            noiseMode: p[0] === 0 ? 0 : p[0] === 0x0a ? 0x03 : 0x01,
            adaptive: false,
          };
        }
        const s = this.state.ancScene;
        this.emitRx(respond(Cmd.AncSetting, [s.mode, s.subScene, s.noiseValue]));
        this.push();
        return;
      }
      case Cmd.LowLatency: {
        if (p.length) this.state = { ...this.state, gameMode: parseEnable(p[0]!) };
        this.emitRx(respond(Cmd.LowLatency, [enableByte(this.state.gameMode ? "on" : "off")]));
        this.push();
        return;
      }
      case Cmd.InEarDetection: {
        if (p.length) this.state = { ...this.state, inEar: parseEnable(p[0]!) };
        this.emitRx(respond(Cmd.InEarDetection, [enableByte(this.state.inEar ? "on" : "off")]));
        this.push();
        return;
      }
      case Cmd.SleepMode: {
        if (p.length) this.state = { ...this.state, sleepMode: parseEnable(p[0]!) };
        this.emitRx(respond(Cmd.SleepMode, [enableByte(this.state.sleepMode ? "on" : "off")]));
        this.push();
        return;
      }
      case Cmd.SpatialAudio: {
        if (p.length) this.state = { ...this.state, spatial: parseEnable(p[0]!) };
        this.emitRx(respond(Cmd.SpatialAudio, [enableByte(this.state.spatial ? "on" : "off")]));
        this.push();
        return;
      }
      case Cmd.EnvAdaptation: {
        if (p.length) {
          const on = parseEnable(p[0]!);
          this.state = { ...this.state, adaptive: on, noiseMode: on ? 0x01 : this.state.noiseMode };
        }
        this.emitRx(respond(Cmd.EnvAdaptation, [enableByte(this.state.adaptive ? "on" : "off")]));
        this.push();
        return;
      }
      case Cmd.Ldac: {
        if (p.length) {
          const on = parseEnable(p[0]!);
          this.state = {
            ...this.state,
            ldacRequested: on,
            audio: {
              ...this.state.audio,
              codec: on ? "LDAC" : "AAC",
              sampleRate: on ? 96000 : 48000,
            },
          };
        }
        this.emitRx(respond(Cmd.Ldac, [enableByte(this.state.ldacRequested ? "on" : "off")]));
        this.push();
        return;
      }
      case Cmd.EqParamsV2: {
        if (p.length >= 3) {
          const parsed = parseEqV2(p);
          if (parsed) this.state = { ...this.state, eq: parsed, eqName: "Custom" };
        }
        this.emitRx(encodeCommand(Cmd.EqParamsV2, eqToParams(this.state.eq)));
        this.push();
        return;
      }
      case Cmd.WearingDetection: {
        if (p.length >= 3) {
          this.state = {
            ...this.state,
            wear: {
              enabled: p[0] === 0x01,
              musicIndex: p[1]!,
              ancIndex: p[2]!,
              toneEnable: p.length >= 4 ? p[3] === 0x01 : this.state.wear.toneEnable,
            },
          };
        }
        const w = this.state.wear;
        this.emitRx(
          respond(Cmd.WearingDetection, [
            w.enabled ? 0x01 : 0x02,
            w.musicIndex,
            w.ancIndex,
            w.toneEnable ? 0x01 : 0x02,
          ]),
        );
        this.push();
        return;
      }
      case Cmd.SoundBalance: {
        if (p.length) this.state = { ...this.state, soundBalance: p[0]! };
        this.emitRx(respond(Cmd.SoundBalance, [this.state.soundBalance]));
        this.push();
        return;
      }
      case Cmd.LightFlash:
      case Cmd.TonePlay:
        this.emitRx(respond(block.cmd, p.length ? [p[0]!] : [0x01]));
        return;
      case Cmd.MusicControl: {
        const action = p[0] ?? 0;
        let media = { ...this.state.media };
        if (action === 0x01) media = { ...media, playing: true };
        if (action === 0x02) media = { ...media, playing: false };
        if (action === 0x03) media = { ...media, title: "Previous track" };
        if (action === 0x04) media = { ...media, title: "Next track" };
        this.state = { ...this.state, media };
        this.emitRx(respond(Cmd.MusicControl, [action]));
        this.push();
        return;
      }
      case Cmd.KeyFunction:
        this.emitRx(encodeCommand(Cmd.KeyFunction, encodeKeyFunctionDirect(this.state.bindings)));
        return;
      case Cmd.RenameDevice: {
        if (p.length) {
          const name = new TextDecoder().decode(p).replace(/\0+$/, "");
          this.state = { ...this.state, name };
        }
        this.emitRx(encodeCommand(Cmd.RenameDevice, [...new TextEncoder().encode(this.state.name), 0]));
        this.push();
        return;
      }
      default:
        this.emitRx(respond(block.cmd, p.length ? p : [0x00]));
    }
  }

  private emitRx(bytes: Uint8Array) {
    const decoded = decodePacket(bytes);
    const summary = decoded.ok ? decoded.packet.blocks.map((b) => cmdName(b.cmd)).join(",") : "bad";
    this.emitLog("rx", bytes, summary, decoded.ok ? (decoded.packet.blocks[0]?.cmd ?? 0) : 0);
  }

  private emitLog(dir: "tx" | "rx", bytes: Uint8Array, summary: string, cmd: number) {
    this.events?.onLog({
      id: logSeq++,
      at: Date.now(),
      dir,
      hex: Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join(" "),
      summary,
      cmd,
    });
  }

  private push() {
    this.events?.onState({ ...this.state });
  }

  private startRssi() {
    this.stopRssi();
    this.rssiTimer = setInterval(() => {
      const jitter = (Math.random() - 0.5) * 6;
      this.smoothRssi = this.smoothRssi * 0.82 + (this.smoothRssi + jitter) * 0.18;
      const rssi = Math.round(Math.max(-90, Math.min(-30, this.smoothRssi)));
      this.state = { ...this.state, rssi };
      this.push();
    }, 1600);
  }

  private stopRssi() {
    if (this.rssiTimer) {
      clearInterval(this.rssiTimer);
      this.rssiTimer = null;
    }
  }
}

type BluetoothLEScanFilter = {
  services?: string[];
  namePrefix?: string;
  manufacturerId?: number;
};

type RequestDeviceOptions = {
  filters?: BluetoothLEScanFilter[];
  optionalServices?: string[];
  acceptAllDevices?: boolean;
};

type BluetoothRemoteGATTCharacteristic = {
  uuid: string;
  writeValue: (data: BufferSource) => Promise<void>;
  readValue: () => Promise<DataView>;
  startNotifications: () => Promise<BluetoothRemoteGATTCharacteristic>;
  addEventListener: (type: string, listener: (ev: Event) => void) => void;
};

type BluetoothRemoteGATTService = {
  getCharacteristic: (uuid: string) => Promise<BluetoothRemoteGATTCharacteristic>;
};

type BluetoothRemoteGATTServer = {
  connected: boolean;
  connect: () => Promise<BluetoothRemoteGATTServer>;
  disconnect: () => void;
  getPrimaryService: (uuid: string) => Promise<BluetoothRemoteGATTService>;
};

type BluetoothDevice = {
  id: string;
  name?: string;
  gatt?: BluetoothRemoteGATTServer;
  addEventListener: (type: string, listener: () => void) => void;
};

type BluetoothNavigator = Navigator & {
  bluetooth?: {
    requestDevice: (options: RequestDeviceOptions) => Promise<BluetoothDevice>;
  };
};

export function webBluetoothAvailable(): boolean {
  return typeof navigator !== "undefined" && Boolean((navigator as BluetoothNavigator).bluetooth);
}

/**
 * Reduce a decoded notification/response packet into typed device state (issue #2).
 * Only fields with an evidenced parser are applied; anything unrecognized is ignored so
 * an unproven frame can never corrupt state. Reports whether proven telemetry changed so
 * callers can flip `telemetryKnown` and emit `onState`.
 */
export function reducePacketIntoState(
  state: DeviceLiveState,
  blocks: CommandBlock[],
): { state: DeviceLiveState; changed: boolean } {
  let next = state;
  let changed = false;
  for (const block of blocks) {
    const p = block.params;
    switch (block.cmd) {
      case Cmd.Battery: {
        const battery = parseBatteryBytes(p);
        if (battery) {
          next = { ...next, battery, telemetryKnown: true };
          changed = true;
        }
        break;
      }
      case Cmd.Version: {
        const firmware = parseFirmwareBytes(p);
        if (firmware) {
          next = { ...next, firmware, telemetryKnown: true };
          changed = true;
        }
        break;
      }
      case Cmd.AncSetting: {
        const ancScene = parseAncScene(p);
        if (ancScene) {
          next = { ...next, ancScene };
          changed = true;
        }
        break;
      }
      case Cmd.NoiseCancelMode: {
        if (p.length >= 1) {
          next = { ...next, noiseMode: p[0]! };
          changed = true;
        }
        break;
      }
      case Cmd.LowLatency: {
        if (p.length >= 1) {
          next = { ...next, gameMode: parseEnable(p[0]!) };
          changed = true;
        }
        break;
      }
      case Cmd.SleepMode: {
        if (p.length >= 1) {
          next = { ...next, sleepMode: parseEnable(p[0]!) };
          changed = true;
        }
        break;
      }
      case Cmd.SpatialAudio: {
        if (p.length >= 1) {
          next = { ...next, spatial: parseEnable(p[0]!) };
          changed = true;
        }
        break;
      }
      case Cmd.InEarDetection: {
        if (p.length >= 1) {
          next = { ...next, inEar: parseEnable(p[0]!) };
          changed = true;
        }
        break;
      }
      case Cmd.SoundBalance: {
        if (p.length >= 1) {
          next = { ...next, soundBalance: p[0]! };
          changed = true;
        }
        break;
      }
      case Cmd.EqParamsV2: {
        const eq = parseEqV2(p);
        if (eq) {
          next = { ...next, eq };
          changed = true;
        }
        break;
      }
      case Cmd.WearingDetection: {
        const wear = parseWear(p);
        if (wear) {
          next = { ...next, wear };
          changed = true;
        }
        break;
      }
      default:
        break;
    }
  }
  return { state: next, changed };
}

/** Minimal injectable Web Bluetooth entry point (used by the fake GATT harness). */
export type WebBluetoothProvider = {
  requestDevice: (options: RequestDeviceOptions) => Promise<BluetoothDevice>;
};

export class WebBluetoothTransport implements QcyTransport {
  kind: TransportKind = "web-bluetooth";
  private device: BluetoothDevice | null = null;
  private server: BluetoothRemoteGATTServer | null = null;
  private chars = new Map<string, BluetoothRemoteGATTCharacteristic>();
  private events: TransportEvents | null = null;
  private optIn: SessionOptIn = { ...DEFAULT_OPT_IN };
  private connectInFlight = false;

  /**
   * @param bluetooth Optional injectable provider (fake GATT test harness). Defaults to
   * `navigator.bluetooth` when available.
   */
  constructor(private readonly bluetooth?: WebBluetoothProvider) {}

  private provider(): WebBluetoothProvider | undefined {
    if (this.bluetooth) return this.bluetooth;
    if (typeof navigator === "undefined") return undefined;
    return (navigator as BluetoothNavigator).bluetooth;
  }

  setExperimentalOptIn(on: boolean): void {
    this.optIn = { ...this.optIn, experimental: on };
  }

  // Real sessions start with unknown/unobserved telemetry, not mock values.
  private state = createRealInitialState();

  async scan(): Promise<DiscoveredDevice[]> {
    const bt = this.provider();
    if (!bt) throw new Error("Web Bluetooth is not available in this browser.");
    const device = await bt.requestDevice({
      filters: [{ namePrefix: "QCY" }, { namePrefix: "MeloBuds" }],
      optionalServices: [SERVICE.main],
    });
    this.device = device;
    const name = device.name ?? "QCY device";
    return [
      {
        id: device.id,
        name,
        address: device.id,
        rssi: -60,
        advertisement: null,
        profile: identifyProfile({ name }),
        kind: "web-bluetooth",
      },
    ];
  }

  async connect(id: string, events: TransportEvents): Promise<void> {
    if (this.connectInFlight) return; // idempotent
    this.connectInFlight = true;
    this.events = events;
    try {
      const device = this.device;
      if (!device?.gatt) throw new Error("No Bluetooth device selected.");
      this.state = {
        ...this.state,
        connecting: true,
        name: device.name ?? "QCY device",
        address: id,
        profile: identifyProfile({ name: device.name }),
      };
      events.onState(this.state);
      this.server = await device.gatt.connect();
      const service = await this.server.getPrimaryService(SERVICE.main);
      // Resolve and cache the required characteristics.
      const required = [
        CHAR.commandWrite,
        CHAR.settingsNotify,
        CHAR.battery,
        CHAR.version,
        CHAR.eqDirect,
        CHAR.keyFunctionV2,
      ];
      for (const uuid of required) {
        try {
          this.chars.set(uuid, await service.getCharacteristic(uuid));
        } catch {
          // Characteristic not present on this device; leave unresolved.
        }
      }
      const notify = this.chars.get(CHAR.settingsNotify);
      if (notify) {
        await notify.startNotifications();
        notify.addEventListener("characteristicvaluechanged", (ev) => this.handleNotification(ev));
      }
      device.addEventListener("gattserverdisconnected", () => {
        this.state = { ...this.state, connected: false, connecting: false };
        this.events?.onDisconnected("gatt-disconnected");
      });
      this.state = { ...this.state, connecting: false, connected: true };
      events.onState(this.state);
      await this.initialSync();
    } catch (err) {
      this.state = { ...this.state, connecting: false, connected: false };
      this.events?.onState(this.state);
      throw err;
    } finally {
      this.connectInFlight = false;
    }
  }

  private handleNotification(ev: Event): void {
    const target = ev.target as unknown as { value?: DataView };
    const view = target.value;
    if (!view) return;
    const bytes = new Uint8Array(view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength));
    const decoded = decodePacket(bytes);
    this.events?.onLog({
      id: logSeq++,
      at: Date.now(),
      dir: "rx",
      hex: Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join(" "),
      summary: decoded.ok ? decoded.packet.blocks.map((b) => cmdName(b.cmd)).join(",") : "bad-frame",
      cmd: decoded.ok ? (decoded.packet.blocks[0]?.cmd ?? 0) : 0,
    });
    if (decoded.ok) {
      const { state, changed } = reducePacketIntoState(this.state, decoded.packet.blocks);
      if (changed) {
        this.state = state;
        this.events?.onState(this.state);
      }
    }
  }

  /** Documented initial state sync after connect: read battery and firmware. */
  private async initialSync(): Promise<void> {
    let changed = false;
    try {
      const batteryBytes = await this.read(CHAR.battery);
      if (batteryBytes.length) {
        const battery = parseBatteryBytes(batteryBytes);
        if (battery) {
          this.state = { ...this.state, battery, telemetryKnown: true };
          changed = true;
        }
      }
    } catch {
      /* best-effort */
    }
    try {
      const fwBytes = await this.read(CHAR.version);
      if (fwBytes.length) {
        const firmware = parseFirmwareBytes(fwBytes);
        if (firmware) {
          this.state = { ...this.state, firmware };
          changed = true;
        }
      }
    } catch {
      /* best-effort */
    }
    if (changed) this.events?.onState(this.state);
  }

  async disconnect(): Promise<void> {
    this.chars.clear();
    this.server = null;
    try {
      this.device?.gatt?.disconnect();
    } catch {
      // Already disconnected; keep the transition idempotent.
    }
    this.state = { ...this.state, connected: false, connecting: false };
    this.events?.onDisconnected("disconnected");
  }

  async write(bytes: Uint8Array): Promise<void> {
    const auth = authorizeFrameWrite(this.state.profile, this.optIn, bytes);
    if (!auth.ok) throw new WriteDeniedError(auth.denial);
    const char = this.chars.get(CHAR.commandWrite);
    if (!char) throw new Error("Not connected");
    this.events?.onLog({
      id: logSeq++,
      at: Date.now(),
      dir: "tx",
      hex: Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join(" "),
      summary: "write",
      cmd: bytes[2] ?? 0,
    });
    await char.writeValue(new Uint8Array(bytes));
  }

  async writeDirect(charUuid: string, bytes: Uint8Array): Promise<void> {
    const auth = authorizeDirectWrite(this.state.profile, this.optIn, charUuid, bytes);
    if (!auth.ok) throw new WriteDeniedError(auth.denial);
    const char = await this.resolveChar(charUuid);
    if (!char) throw new Error(`Characteristic ${charUuid} is not available`);
    this.events?.onLog({
      id: logSeq++,
      at: Date.now(),
      dir: "tx",
      hex: Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join(" "),
      summary: `direct ${charUuid.slice(4, 8)}`,
      cmd: 0,
    });
    await char.writeValue(new Uint8Array(bytes));
  }

  async read(charUuid: string): Promise<Uint8Array> {
    const char = await this.resolveChar(charUuid);
    if (!char) return new Uint8Array();
    const view = await char.readValue();
    return new Uint8Array(view.buffer.slice(view.byteOffset, view.byteOffset + view.byteLength));
  }

  private async resolveChar(uuid: string): Promise<BluetoothRemoteGATTCharacteristic | null> {
    const cached = this.chars.get(uuid);
    if (cached) return cached;
    if (!this.server) return null;
    try {
      const service = await this.server.getPrimaryService(SERVICE.main);
      const char = await service.getCharacteristic(uuid);
      this.chars.set(uuid, char);
      return char;
    } catch {
      return null;
    }
  }
}
