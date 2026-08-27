import { beforeAll, beforeEach, describe, expect, it } from "vitest";
import { transportForTests, useHub } from "./hub-store";
import type { SmartProfile } from "./smart-profiles";
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
