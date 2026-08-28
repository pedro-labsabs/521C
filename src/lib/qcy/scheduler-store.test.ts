import { beforeAll, beforeEach, describe, expect, it } from "vitest";
import { currentNoiseUi, transportForTests, useHub } from "./hub-store";
import type { SmartProfile } from "./smart-profiles";
import { Cmd, set } from "./protocol";
import { MockTransport } from "./transport";

describe("applyProfile structured results (issue #10)", () => {
  beforeAll(async () => {
    await useHub.getState().scan();
    const id = useHub.getState().discovered[0]!.id;
    await useHub.getState().connect(id);
  });

  beforeEach(() => {
    useHub.setState({ experimentalOptIn: false });
  });

  it("reports every step as succeeded for a supported built-in profile", async () => {
    const result = await useHub.getState().applyProfile("music");
    expect(result).not.toBeNull();
    expect(result!.ok).toBe(true);
    expect(result!.steps.every((s) => s.ok)).toBe(true);
    expect(result!.steps.map((s) => s.step)).toContain("noise");
    expect(result!.steps.map((s) => s.step)).toContain("gameMode");
    expect(result!.observed).toHaveProperty("noiseMode");
    expect(result!.observed).toHaveProperty("eqName");
  });

  it("flags the failed step and still reports the rest on partial failure", async () => {
    // ANC writes are hardware-validated 0x17 now (no policy-driven failure left
    // in the default flow), so the failure is injected deterministically at the
    // mock transport boundary: opcode 0x17 rejects -> the noise step fails
    // while the other steps still apply.
    const fixture: SmartProfile = {
      id: "test-partial",
      name: "Test Partial",
      description: "partial failure fixture",
      builtin: false,
      noise: "anc",
      ancLevel: 2,
      transparencyLevel: 4,
      gameMode: true,
      eqId: "flat",
      wearDetection: true,
    };
    useHub.getState().saveCustomProfile(fixture);
    const t = transportForTests() as MockTransport;
    t.failOpcode = 0x17;
    try {
      const result = await useHub.getState().applyProfile("test-partial");
      expect(result).not.toBeNull();
      expect(result!.ok).toBe(false);
      const noise = result!.steps.find((s) => s.step === "noise");
      expect(noise?.ok).toBe(false);
      const game = result!.steps.find((s) => s.step === "gameMode");
      expect(game?.ok).toBe(true);
    } finally {
      t.failOpcode = null;
    }
  });

  it("returns null for an unknown profile id", async () => {
    const result = await useHub.getState().applyProfile("does-not-exist");
    expect(result).toBeNull();
  });
});

describe("coalesced latest-value controls through the store", () => {
  beforeAll(async () => {
    await useHub.getState().scan();
    const id = useHub.getState().discovered[0]!.id;
    await useHub.getState().connect(id);
  });

  it("settles sound balance on the latest requested value", async () => {
    const calls = [30, 45, 60, 75, 90];
    await Promise.all(calls.map((v) => useHub.getState().setSoundBalance(v)));
    expect(useHub.getState().device.soundBalance).toBe(90);
  });
});

/* ------------------------------------------------------------------ */
/* Read-before-write gating for unknown device state (#62)             */
/* ------------------------------------------------------------------ */

describe("store gating while device state is unknown (#62)", () => {
  beforeAll(async () => {
    await useHub.getState().scan();
    const id = useHub.getState().discovered[0]!.id;
    await useHub.getState().connect(id);
  });

  it("currentNoiseUi is null while the ANC scene is unknown", () => {
    const d = useHub.getState().device;
    useHub.setState({ device: { ...d, ancScene: null, noiseMode: -1 } });
    try {
      expect(currentNoiseUi(useHub.getState().device)).toBeNull();
      // A known scene decodes as before.
      expect(
        currentNoiseUi({
          ...d,
          ancScene: { mode: 0x01, subScene: 0x04, noiseValue: 0x00 },
        }),
      ).toBe("wind");
    } finally {
      useHub.setState({ device: d });
    }
  });

  it("setBinding refuses while the bindings table is unknown", async () => {
    const d = useHub.getState().device;
    useHub.setState({ device: { ...d, bindings: null }, toast: null });
    try {
      await useHub.getState().setBinding(0x01, 0x03);
      const s = useHub.getState();
      // No write happened: the table is still unknown, not fabricated.
      expect(s.device.bindings).toBeNull();
      expect(s.toast?.title).toBe("Write blocked");
    } finally {
      useHub.setState({ device: d, toast: null });
    }
  });

  it("setBinding merges into the table read from the device when known", async () => {
    const d = useHub.getState().device;
    const table = [
      { keyId: 0x01, funId: 0x02 },
      { keyId: 0x02, funId: 0x09 },
    ];
    useHub.setState({ device: { ...d, bindings: table }, toast: null });
    try {
      await useHub.getState().setBinding(0x01, 0x05);
      const bindings = useHub.getState().device.bindings;
      expect(bindings).toEqual([
        { keyId: 0x01, funId: 0x05 },
        { keyId: 0x02, funId: 0x09 }, // untouched key keeps the read value
      ]);
    } finally {
      useHub.setState({ device: d, toast: null });
    }
  });

  it("setWear refuses while wear settings are unknown", async () => {
    const d = useHub.getState().device;
    useHub.setState({ device: { ...d, wear: null }, toast: null });
    try {
      await useHub.getState().setWear({ enabled: false });
      const s = useHub.getState();
      expect(s.device.wear).toBeNull();
      expect(s.toast?.title).toBe("Write blocked");
    } finally {
      useHub.setState({ device: d, toast: null });
    }
  });

  it("chime preflight treats unknown worn state as unknown, never not-worn", async () => {
    const d = useHub.getState().device;
    useHub.setState({
      device: { ...d, wear: null, wornLeft: null, wornRight: null },
      pendingChime: null,
    });
    try {
      useHub.getState().requestChime("left");
      const pre = useHub.getState().pendingChime;
      expect(pre?.status).toBe("confirm-strong");
      expect(pre?.unknownTargets).toEqual(["left"]);
    } finally {
      useHub.setState({ device: d, pendingChime: null });
    }
  });
});

/* ------------------------------------------------------------------ */
/* Mock transport mirrors the falsified 0x0C hardware behavior (#71)   */
/* ------------------------------------------------------------------ */

describe("mock transport ignores falsified 0x0C (#71)", () => {
  beforeAll(async () => {
    await useHub.getState().scan();
    const id = useHub.getState().discovered[0]!.id;
    await useHub.getState().connect(id);
  });

  it("applies no state change and emits no ACK for 0x0C writes", async () => {
    const t = transportForTests() as MockTransport;
    t.setExperimentalOptIn(true);
    try {
      // Settle a known scene-derived noiseMode first.
      await t.write(set.ancSetting({ mode: 0x01, subScene: 0x03, noiseValue: 0x02 }));
      const noiseBefore = useHub.getState().device.noiseMode;
      expect(noiseBefore).toBe(0x01);
      const rx0cBefore = useHub
        .getState()
        .log.filter((e) => e.dir === "rx" && e.cmd === Cmd.NoiseCancelMode).length;

      await t.write(set.noiseMode(0x00)); // live HT08 ignores this

      const rx0cAfter = useHub
        .getState()
        .log.filter((e) => e.dir === "rx" && e.cmd === Cmd.NoiseCancelMode).length;
      expect(rx0cAfter).toBe(rx0cBefore); // no fake ACK

      // A later supported write pushes state; the ignored 0x0C payload must
      // not have mutated it.
      await t.write(set.lowLatency("on"));
      expect(useHub.getState().device.noiseMode).toBe(noiseBefore);
    } finally {
      t.setExperimentalOptIn(false);
    }
  });

  it("policy still gates 0x0C behind the experimental opt-in", async () => {
    const t = transportForTests() as MockTransport;
    t.setExperimentalOptIn(false);
    await expect(t.write(set.noiseMode(0x01))).rejects.toThrow();
  });
});
