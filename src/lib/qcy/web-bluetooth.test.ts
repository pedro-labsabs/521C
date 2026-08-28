import { describe, expect, it } from "vitest";
import {
  CHAR,
  Cmd,
  decodePacket,
  encodeBatteryBytes,
  encodeCommand,
  encodeKeyFunctionDirect,
  set,
} from "./protocol";
import {
  WebBluetoothTransport,
  createRealInitialState,
  reducePacketIntoState,
  type WebBluetoothProvider,
} from "./transport";

/* ------------------------------------------------------------------ */
/* Fake GATT harness                                                    */
/* ------------------------------------------------------------------ */

type FakeChar = {
  uuid: string;
  value: Uint8Array;
  writes: Uint8Array[];
  listeners: Map<string, Array<(ev: unknown) => void>>;
  writeValue: (data: BufferSource) => Promise<void>;
  readValue: () => Promise<DataView>;
  startNotifications: () => Promise<FakeChar>;
  addEventListener: (type: string, listener: (ev: unknown) => void) => void;
  emit: (bytes: Uint8Array) => void;
};

function makeChar(uuid: string, value: Uint8Array = new Uint8Array()): FakeChar {
  const listeners = new Map<string, Array<(ev: unknown) => void>>();
  const char: FakeChar = {
    uuid,
    value,
    writes: [],
    listeners,
    async writeValue(data: BufferSource) {
      const src = data instanceof Uint8Array ? data : new Uint8Array(data as ArrayBuffer);
      char.writes.push(new Uint8Array(src));
    },
    async readValue() {
      return new DataView(char.value.buffer, char.value.byteOffset, char.value.byteLength);
    },
    async startNotifications() {
      return char;
    },
    addEventListener(type, listener) {
      const arr = listeners.get(type) ?? [];
      arr.push(listener);
      listeners.set(type, arr);
    },
    emit(bytes: Uint8Array) {
      char.value = bytes;
      const ev = { target: { value: new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength) } };
      for (const l of listeners.get("characteristicvaluechanged") ?? []) l(ev);
    },
  };
  return char;
}

type FakeDeviceCtl = {
  device: unknown;
  chars: Map<string, FakeChar>;
  server: { connected: boolean };
  /** Mutable failure switches a test can flip mid-session. */
  flags: { failConnect: boolean; noService: boolean };
  /** Fire a device-level event, e.g. "gattserverdisconnected". */
  fire: (type: string) => void;
};

function makeFakeDevice(opts: { id?: string; name?: string } = {}): FakeDeviceCtl {
  const flags = { failConnect: false, noService: false };
  const battery = encodeBatteryBytes({
    left: { level: 55, charging: false },
    right: { level: 56, charging: false },
    case: { level: 77, charging: false },
  });
  // A device-shaped touch table so the read-before-write path (#62) has
  // something real to read; tests that want an unreadable table delete the
  // characteristic before connecting.
  const keyTable = encodeKeyFunctionDirect([
    { keyId: 0x01, funId: 0x02 },
    { keyId: 0x02, funId: 0x02 },
    { keyId: 0x03, funId: 0x04 },
  ]);
  const chars = new Map<string, FakeChar>([
    [CHAR.commandWrite, makeChar(CHAR.commandWrite)],
    [CHAR.settingsNotify, makeChar(CHAR.settingsNotify)],
    [CHAR.battery, makeChar(CHAR.battery, battery)],
    [CHAR.version, makeChar(CHAR.version, Uint8Array.from([1, 4, 2]))],
    [CHAR.eqDirect, makeChar(CHAR.eqDirect)],
    [CHAR.keyFunctionV2, makeChar(CHAR.keyFunctionV2, keyTable)],
  ]);
  const service = {
    async getCharacteristic(uuid: string) {
      const c = chars.get(uuid);
      if (!c) throw new Error(`no characteristic ${uuid}`);
      return c;
    },
  };
  const server = {
    connected: false,
    async connect() {
      if (flags.failConnect) throw new Error("gatt connect failed");
      server.connected = true;
      return server;
    },
    disconnect() {
      server.connected = false;
    },
    async getPrimaryService(_uuid: string) {
      if (flags.noService) throw new Error("no such service");
      return service;
    },
  };
  const listeners = new Map<string, Array<() => void>>();
  const device = {
    id: opts.id ?? "fake-ht08",
    name: opts.name ?? "QCY MeloBuds Pro",
    gatt: server,
    addEventListener(type: string, listener: () => void) {
      const arr = listeners.get(type) ?? [];
      arr.push(listener);
      listeners.set(type, arr);
    },
  };
  return {
    device,
    chars,
    server,
    flags,
    fire: (type: string) => {
      for (const l of listeners.get(type) ?? []) l();
    },
  };
}

function makeHarness() {
  const ctl = makeFakeDevice();
  const provider = {
    async requestDevice(_options: unknown) {
      return ctl.device;
    },
  } as unknown as WebBluetoothProvider;
  return { transport: new WebBluetoothTransport(provider), chars: ctl.chars, server: ctl.server, ctl };
}

function makeEvents() {
  const states: Array<Record<string, unknown>> = [];
  const logs: Array<Record<string, unknown>> = [];
  const disconnects: string[] = [];
  return {
    states,
    logs,
    disconnects,
    onState: (s: Record<string, unknown>) => {
      states.push(s);
    },
    onLog: (e: Record<string, unknown>) => {
      logs.push(e);
    },
    onDisconnected: (r: string) => {
      disconnects.push(r);
    },
  };
}

/* ------------------------------------------------------------------ */
/* reducePacketIntoState (notification -> typed state)                  */
/* ------------------------------------------------------------------ */

describe("reducePacketIntoState", () => {
  it("reduces proven battery telemetry and flips telemetryKnown", () => {
    const state = createRealInitialState();
    expect(state.telemetryKnown).toBe(false);
    const block = { cmd: Cmd.Battery, params: Uint8Array.from([10, 20, 30]) };
    const { state: next, changed } = reducePacketIntoState(state, [block]);
    expect(changed).toBe(true);
    expect(next.battery.left.level).toBe(10);
    expect(next.battery.right.level).toBe(20);
    expect(next.battery.case.level).toBe(30);
    expect(next.telemetryKnown).toBe(true);
  });

  it("reduces firmware and enable-type fields", () => {
    const state = createRealInitialState();
    const { state: next } = reducePacketIntoState(state, [
      { cmd: Cmd.Version, params: Uint8Array.from([2, 0, 1]) },
      { cmd: Cmd.LowLatency, params: Uint8Array.from([1]) },
      { cmd: Cmd.SoundBalance, params: Uint8Array.from([0x40]) },
    ]);
    expect(next.firmware.left).toBe("2.0.1");
    expect(next.gameMode).toBe(true);
    expect(next.soundBalance).toBe(0x40);
  });

  it("ignores unrecognized commands without corrupting state", () => {
    const state = createRealInitialState();
    const { state: next, changed } = reducePacketIntoState(state, [
      { cmd: 0x7f, params: Uint8Array.from([1, 2, 3]) },
    ]);
    expect(changed).toBe(false);
    expect(next).toEqual(state);
  });
});

/* ------------------------------------------------------------------ */
/* Real sessions start unknown and read before write (#62)             */
/* ------------------------------------------------------------------ */

describe("real session unknown state (#62)", () => {
  it("createRealInitialState marks wear/scene/bindings/worn as unknown", () => {
    const state = createRealInitialState();
    expect(state.wornLeft).toBeNull();
    expect(state.wornRight).toBeNull();
    expect(state.wear).toBeNull();
    expect(state.ancScene).toBeNull();
    expect(state.bindings).toBeNull();
    expect(state.noiseMode).toBe(-1);
    expect(state.telemetryKnown).toBe(false);
  });

  it("mock initial state stays fully populated (unchanged behavior)", async () => {
    const { createInitialState } = await import("./transport");
    const state = createInitialState();
    expect(state.wornLeft).toBe(true);
    expect(state.wornRight).toBe(true);
    expect(state.wear).not.toBeNull();
    expect(state.ancScene).not.toBeNull();
    expect(state.bindings!.length).toBeGreaterThan(0);
  });

  it("initialSync reads the touch table and requests wear + ANC scene state", async () => {
    const { transport, chars } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const last = events.states[events.states.length - 1]!;
    // Bindings came from the device's characteristic, not from app defaults.
    expect(last.bindings).toEqual([
      { keyId: 0x01, funId: 0x02 },
      { keyId: 0x02, funId: 0x02 },
      { keyId: 0x03, funId: 0x04 },
    ]);
    // Wear (0x2C) and ANC scene (0x17) were requested framed via RequestData.
    const requested = chars
      .get(CHAR.commandWrite)!
      .writes.map((w) => decodePacket(w))
      .filter((d) => d.ok && d.packet.blocks[0]?.cmd === Cmd.RequestData)
      .map((d) => (d.ok ? d.packet.blocks[0]!.params[0] : undefined));
    expect(requested).toContain(Cmd.WearingDetection);
    expect(requested).toContain(Cmd.AncSetting);
    // Not answered yet by this fake device: the slices stay unknown.
    expect(last.wear).toBeNull();
    expect(last.ancScene).toBeNull();
  });

  it("missing key characteristic leaves bindings unknown instead of defaulting", async () => {
    const ctl = makeFakeDevice();
    ctl.chars.delete(CHAR.keyFunctionV2);
    const provider = {
      async requestDevice(_options: unknown) {
        return ctl.device;
      },
    } as unknown as WebBluetoothProvider;
    const transport = new WebBluetoothTransport(provider);
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const last = events.states[events.states.length - 1]!;
    expect(last.bindings).toBeNull();
    expect(last.wear).toBeNull();
    expect(last.ancScene).toBeNull();
  });

  it("device answers to the state requests reduce into known wear/scene", async () => {
    const { transport, chars } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const notify = chars.get(CHAR.settingsNotify)!;
    notify.emit(encodeCommand(Cmd.WearingDetection, [0x02, 0x03, 0x01, 0x02]));
    notify.emit(encodeCommand(Cmd.AncSetting, [0x01, 0x04, 0x00]));
    const last = events.states[events.states.length - 1]!;
    expect(last.wear).toEqual({
      enabled: false,
      musicIndex: 0x03,
      ancIndex: 0x01,
      toneEnable: false,
    });
    expect(last.ancScene).toEqual({ mode: 0x01, subScene: 0x04, noiseValue: 0x00 });
  });
});

/* ------------------------------------------------------------------ */
/* WebBluetoothTransport against the fake GATT harness                  */
/* ------------------------------------------------------------------ */

describe("WebBluetoothTransport (fake GATT)", () => {
  it("starts with unknown telemetry, then syncs battery/firmware on connect", async () => {
    const { transport } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const last = events.states[events.states.length - 1]!;
    expect(last.connected).toBe(true);
    expect(last.telemetryKnown).toBe(true);
    expect((last.battery as { left: { level: number } }).left.level).toBe(55);
    expect((last.firmware as { left: string }).left).toBe("1.4.2");
  });

  it("reads a specific allowlisted characteristic", async () => {
    const { transport } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const bytes = await transport.read(CHAR.battery);
    expect(bytes.length).toBeGreaterThanOrEqual(3);
    expect(bytes[0]).toBe(55);
  });

  it("reduces a battery notification into device state", async () => {
    const { transport, chars } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const before = events.states.length;
    chars.get(CHAR.settingsNotify)!.emit(encodeCommand(Cmd.Battery, [5, 6, 7]));
    const last = events.states[events.states.length - 1]!;
    expect(events.states.length).toBeGreaterThan(before);
    expect((last.battery as { left: { level: number } }).left.level).toBe(5);
  });

  it("writes frames to the command characteristic", async () => {
    const { transport, chars } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    await transport.write(set.lowLatency("on"));
    const cmdChar = chars.get(CHAR.commandWrite)!;
    // initialSync already requested wear (0x2C) and ANC scene (0x17) state
    // via framed RequestData (#62); the caller's frame is the next write.
    const syncFrames = cmdChar.writes.filter((w) => w[2] === Cmd.RequestData);
    expect(syncFrames.length).toBe(2);
    const last = cmdChar.writes[cmdChar.writes.length - 1]!;
    expect(last[2]).toBe(Cmd.LowLatency);
  });

  it("writeDirect targets the requested allowlisted characteristic", async () => {
    const { transport, chars } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const framedBefore = chars.get(CHAR.commandWrite)!.writes.length;
    const payload = Uint8Array.from([1, 2, 3, 4]);
    await transport.writeDirect(CHAR.keyFunctionV2, payload);
    expect(chars.get(CHAR.keyFunctionV2)!.writes.length).toBe(1);
    // Only the initialSync RequestData frames may sit on the command char.
    expect(chars.get(CHAR.commandWrite)!.writes.length).toBe(framedBefore);
  });

  it("failed reconnect invalidates the previous session", async () => {
    const { transport, ctl } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    ctl.flags.failConnect = true;
    await expect(transport.connect("fake-ht08", events)).rejects.toThrow();
    // Stale handles must not be usable after the failed reconnect.
    await expect(transport.write(set.lowLatency("on"))).rejects.toThrow("Not connected");
    await expect(transport.read(CHAR.battery)).rejects.toThrow("Not connected");
    // No write beyond the first session's initialSync requests may arrive.
    expect(ctl.chars.get(CHAR.commandWrite)!.writes.length).toBeLessThanOrEqual(2);
  });

  it("missing vendor service leaves no session behind", async () => {
    const { transport, ctl } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    ctl.flags.noService = true;
    await expect(transport.connect("fake-ht08", events)).rejects.toThrow();
    await expect(transport.write(set.lowLatency("on"))).rejects.toThrow("Not connected");
    expect(ctl.chars.get(CHAR.commandWrite)!.writes).toHaveLength(0);
  });

  it("remote disconnect invalidates cached characteristics", async () => {
    const { transport, ctl } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const framedAtDisconnect = ctl.chars.get(CHAR.commandWrite)!.writes.length;
    ctl.fire("gattserverdisconnected");
    const last = events.states[events.states.length - 1]!;
    expect(last.connected).toBe(false);
    expect(events.disconnects.length).toBeGreaterThan(0);
    // No write may reach the stale characteristics of the dead session.
    await expect(transport.write(set.lowLatency("on"))).rejects.toThrow("Not connected");
    await expect(
      transport.writeDirect(CHAR.keyFunctionV2, Uint8Array.from([1, 2, 3, 4])),
    ).rejects.toThrow("Not connected");
    expect(ctl.chars.get(CHAR.commandWrite)!.writes.length).toBe(framedAtDisconnect);
    expect(ctl.chars.get(CHAR.keyFunctionV2)!.writes).toHaveLength(0);
  });

  it("replacing the device never reuses the previous session's handles", async () => {
    const ctlA = makeFakeDevice({ id: "dev-a" });
    const ctlB = makeFakeDevice({ id: "dev-b" });
    let current = ctlA;
    const provider = {
      async requestDevice(_options: unknown) {
        return current.device;
      },
    } as unknown as WebBluetoothProvider;
    const transport = new WebBluetoothTransport(provider);
    const events = makeEvents();
    await transport.scan();
    await transport.connect("dev-a", events);
    current = ctlB;
    await transport.scan();
    await transport.connect("dev-b", events);
    // A's session ended when B connected: record what A legitimately saw
    // (its own initialSync requests) and ensure nothing more arrives.
    const aWrites = ctlA.chars.get(CHAR.commandWrite)!.writes.length;
    await transport.write(set.lowLatency("on"));
    // After A -> B replacement, only B's characteristic may receive bytes.
    expect(ctlA.chars.get(CHAR.commandWrite)!.writes.length).toBe(aWrites);
    const bWrites = ctlB.chars.get(CHAR.commandWrite)!.writes;
    expect(bWrites[bWrites.length - 1]![2]).toBe(Cmd.LowLatency);
  });

  it("disconnect is idempotent and clears cached characteristics", async () => {
    const { transport, server } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    await transport.disconnect();
    expect(server.connected).toBe(false);
    expect(events.disconnects.length).toBeGreaterThan(0);
    // After disconnect the session is gone: reads report a disconnected error
    // instead of silently resolving against a stale characteristic.
    await expect(transport.read(CHAR.battery)).rejects.toThrow("Not connected");
    // Second disconnect does not throw.
    await expect(transport.disconnect()).resolves.toBeUndefined();
  });
});
