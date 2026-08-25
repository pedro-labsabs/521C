import { describe, expect, it } from "vitest";
import {
  CHAR,
  Cmd,
  encodeBatteryBytes,
  encodeCommand,
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

function makeHarness() {
  const battery = encodeBatteryBytes({
    left: { level: 55, charging: false },
    right: { level: 56, charging: false },
    case: { level: 77, charging: false },
  });
  const chars = new Map<string, FakeChar>([
    [CHAR.commandWrite, makeChar(CHAR.commandWrite)],
    [CHAR.settingsNotify, makeChar(CHAR.settingsNotify)],
    [CHAR.battery, makeChar(CHAR.battery, battery)],
    [CHAR.version, makeChar(CHAR.version, Uint8Array.from([1, 4, 2]))],
    [CHAR.eqDirect, makeChar(CHAR.eqDirect)],
    [CHAR.keyFunctionV2, makeChar(CHAR.keyFunctionV2)],
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
      server.connected = true;
      return server;
    },
    disconnect() {
      server.connected = false;
    },
    async getPrimaryService(_uuid: string) {
      return service;
    },
  };
  const device = {
    id: "fake-ht08",
    name: "QCY MeloBuds Pro",
    gatt: server,
    addEventListener(_type: string, _listener: () => void) {
      /* no-op */
    },
  };
  const provider = {
    async requestDevice(_options: unknown) {
      return device;
    },
  } as unknown as WebBluetoothProvider;
  return { transport: new WebBluetoothTransport(provider), chars, server };
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
    expect(cmdChar.writes.length).toBe(1);
    expect(cmdChar.writes[0]![2]).toBe(Cmd.LowLatency);
  });

  it("writeDirect targets the requested allowlisted characteristic", async () => {
    const { transport, chars } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    const payload = Uint8Array.from([1, 2, 3, 4]);
    await transport.writeDirect(CHAR.keyFunctionV2, payload);
    expect(chars.get(CHAR.keyFunctionV2)!.writes.length).toBe(1);
    expect(chars.get(CHAR.commandWrite)!.writes.length).toBe(0);
  });

  it("disconnect is idempotent and clears cached characteristics", async () => {
    const { transport, server } = makeHarness();
    const events = makeEvents();
    await transport.scan();
    await transport.connect("fake-ht08", events);
    await transport.disconnect();
    expect(server.connected).toBe(false);
    expect(events.disconnects.length).toBeGreaterThan(0);
    // After disconnect a read resolves to empty rather than throwing.
    const bytes = await transport.read(CHAR.battery);
    expect(bytes.length).toBe(0);
    // Second disconnect does not throw.
    await expect(transport.disconnect()).resolves.toBeUndefined();
  });
});
