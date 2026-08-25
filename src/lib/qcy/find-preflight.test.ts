import { beforeAll, beforeEach, describe, expect, it } from "vitest";
import {
  CHIME_COOLDOWN_MS,
  chimeToneId,
  evaluateChimePreflight,
} from "./find-preflight";
import { useHub } from "./hub-store";
import type { DeviceLiveState } from "./transport";

describe("evaluateChimePreflight (pure decision logic)", () => {
  it("blocks a target known to be worn", () => {
    const pre = evaluateChimePreflight("left", {
      detectionEnabled: true,
      wornLeft: true,
      wornRight: false,
    });
    expect(pre.status).toBe("blocked-worn");
    expect(pre.wornTargets).toEqual(["left"]);
    expect(pre.reason.toLowerCase()).toContain("worn");
  });

  it("confirms normally when the target is known not worn", () => {
    const pre = evaluateChimePreflight("right", {
      detectionEnabled: true,
      wornLeft: true,
      wornRight: false,
    });
    expect(pre.status).toBe("confirm");
    expect(pre.notWornTargets).toEqual(["right"]);
  });

  it("escalates to strong confirmation when wear detection is off (state unknown)", () => {
    const pre = evaluateChimePreflight("left", {
      detectionEnabled: false,
      wornLeft: false,
      wornRight: false,
    });
    expect(pre.status).toBe("confirm-strong");
    expect(pre.unknownTargets).toEqual(["left"]);
  });

  it("both: any worn target blocks the whole action", () => {
    const pre = evaluateChimePreflight("both", {
      detectionEnabled: true,
      wornLeft: false,
      wornRight: true,
    });
    expect(pre.status).toBe("blocked-worn");
    expect(pre.wornTargets).toEqual(["right"]);
    expect(pre.targets).toEqual(["left", "right"]);
  });

  it("both: unknown wear on all targets escalates to strong confirmation", () => {
    const pre = evaluateChimePreflight("both", {
      detectionEnabled: false,
      wornLeft: true,
      wornRight: true,
    });
    expect(pre.status).toBe("confirm-strong");
    expect(pre.unknownTargets).toEqual(["left", "right"]);
  });

  it("both: confirms when neither bud is worn", () => {
    const pre = evaluateChimePreflight("both", {
      detectionEnabled: true,
      wornLeft: false,
      wornRight: false,
    });
    expect(pre.status).toBe("confirm");
    expect(pre.notWornTargets).toEqual(["left", "right"]);
  });

  it("maps tone ids for the documented side mapping", () => {
    expect(chimeToneId("left")).toBe(1);
    expect(chimeToneId("right")).toBe(2);
    expect(chimeToneId("both")).toBe(3);
  });

  it("exposes a positive cooldown", () => {
    expect(CHIME_COOLDOWN_MS).toBeGreaterThan(0);
  });
});

describe("store chime gate (no TX before confirmation)", () => {
  function setWear(wornLeft: boolean, wornRight: boolean, enabled: boolean) {
    const d = useHub.getState().device;
    const patch: Partial<DeviceLiveState> = {
      wornLeft,
      wornRight,
      wear: { ...d.wear, enabled },
    };
    useHub.setState({ device: { ...d, ...patch } });
  }

  function chimeTxCount(): number {
    return useHub
      .getState()
      .log.filter((e) => e.dir === "tx" && (e.cmd === 0x05 || e.cmd === 0x3d)).length;
  }

  function reset() {
    useHub.setState({ chimeBlockedUntil: 0, pendingChime: null });
  }

  beforeAll(async () => {
    await useHub.getState().scan();
    const id = useHub.getState().discovered[0]!.id;
    await useHub.getState().connect(id);
  });

  beforeEach(() => {
    reset();
  });

  it("requestChime stages a preflight and transmits nothing", () => {
    setWear(false, false, true);
    const before = chimeTxCount();
    useHub.getState().requestChime("both");
    expect(useHub.getState().pendingChime).not.toBeNull();
    expect(chimeTxCount()).toBe(before);
  });

  it("blocks a known-worn target and sends nothing on confirm", async () => {
    setWear(true, true, true);
    useHub.getState().requestChime("both");
    expect(useHub.getState().pendingChime!.status).toBe("blocked-worn");
    const before = chimeTxCount();
    await useHub.getState().confirmChime();
    expect(chimeTxCount()).toBe(before);
  });

  it("transmits light-flash + tone only after confirmation when not worn", async () => {
    setWear(false, false, true);
    useHub.getState().requestChime("both");
    expect(useHub.getState().pendingChime!.status).toBe("confirm");
    const before = chimeTxCount();
    await useHub.getState().confirmChime();
    expect(chimeTxCount()).toBe(before + 2);
    expect(useHub.getState().pendingChime).toBeNull();
  });

  it("rate-limits a second chime inside the cooldown window", async () => {
    setWear(false, false, true);
    useHub.getState().requestChime("left");
    await useHub.getState().confirmChime();
    const afterFirst = chimeTxCount();
    expect(afterFirst).toBeGreaterThan(0);

    useHub.getState().requestChime("left");
    await useHub.getState().confirmChime();
    expect(chimeTxCount()).toBe(afterFirst);
  });

  it("cancelChime clears the staged preflight without transmitting", () => {
    setWear(false, false, true);
    const before = chimeTxCount();
    useHub.getState().requestChime("right");
    useHub.getState().cancelChime();
    expect(useHub.getState().pendingChime).toBeNull();
    expect(chimeTxCount()).toBe(before);
  });

  it("CLI find command refuses unattended chime", async () => {
    const out = await useHub.getState().runCli("find both");
    expect(out).toContain("refused");
  });
});
