import { describe, expect, it } from "vitest";
import { CommandScheduler, confirmTransition, type CommandResult } from "./scheduler";

describe("CommandScheduler serialization + ordering", () => {
  it("runs commands one at a time in FIFO order", async () => {
    const s = new CommandScheduler();
    const order: string[] = [];
    const mk = (label: string) =>
      s.schedule({ key: label }, async () => {
        order.push(label);
        return { status: "sent" } as CommandResult;
      });
    await Promise.all([mk("a"), mk("b"), mk("c")]);
    expect(order).toEqual(["a", "b", "c"]);
  });

  it("does not start a queued command until the running one finishes", async () => {
    const s = new CommandScheduler();
    let release!: () => void;
    const blocker = new Promise<void>((r) => (release = r));
    const order: string[] = [];
    const p0 = s.schedule({ key: "blocker" }, async () => {
      await blocker;
      order.push("blocker");
      return { status: "sent" };
    });
    const p1 = s.schedule({ key: "next" }, async () => {
      order.push("next");
      return { status: "sent" };
    });
    // While the blocker runs, "next" must stay queued.
    await Promise.resolve();
    expect(order).toEqual([]);
    expect(s.pending).toBe(1);
    release();
    await Promise.all([p0, p1]);
    expect(order).toEqual(["blocker", "next"]);
  });
});

describe("CommandScheduler coalescing", () => {
  it("supersedes queued latest-value commands with the newest one", async () => {
    const s = new CommandScheduler();
    let release!: () => void;
    const blocker = new Promise<void>((r) => (release = r));
    const order: string[] = [];
    const p0 = s.schedule({ key: "blocker" }, async () => {
      await blocker;
      order.push("blocker");
      return { status: "sent" };
    });
    const eq = (v: string) =>
      s.schedule({ key: "eq", coalesce: true }, async () => {
        order.push(v);
        return { status: "sent" };
      });
    const p1 = eq("eq1");
    const p2 = eq("eq2");
    const p3 = eq("eq3");
    release();
    await p0;
    const [r1, r2, r3] = await Promise.all([p1, p2, p3]);
    expect(r1.status).toBe("coalesced");
    expect(r2.status).toBe("coalesced");
    expect(r3.status).toBe("sent");
    expect(order).toEqual(["blocker", "eq3"]);
  });

  it("keeps separate backlogs for different coalescing keys", async () => {
    const s = new CommandScheduler();
    let release!: () => void;
    const blocker = new Promise<void>((r) => (release = r));
    const order: string[] = [];
    const p0 = s.schedule({ key: "blocker" }, async () => {
      await blocker;
      return { status: "sent" };
    });
    const pa1 = s.schedule({ key: "a", coalesce: true }, async () => {
      order.push("a1");
      return { status: "sent" };
    });
    const pa2 = s.schedule({ key: "a", coalesce: true }, async () => {
      order.push("a2");
      return { status: "sent" };
    });
    const pb1 = s.schedule({ key: "b", coalesce: true }, async () => {
      order.push("b1");
      return { status: "sent" };
    });
    release();
    await Promise.all([p0, pa1, pa2, pb1]);
    expect((await pa1).status).toBe("coalesced");
    expect((await pa2).status).toBe("sent");
    expect((await pb1).status).toBe("sent");
    expect(order).toEqual(["a2", "b1"]);
  });
});

describe("CommandScheduler cancellation", () => {
  it("cancels queued work on disconnect without running it", async () => {
    const s = new CommandScheduler();
    let release!: () => void;
    const blocker = new Promise<void>((r) => (release = r));
    const order: string[] = [];
    const p0 = s.schedule({ key: "blocker" }, async () => {
      await blocker;
      order.push("blocker");
      return { status: "sent" };
    });
    const px = s.schedule({ key: "x" }, async () => {
      order.push("x");
      return { status: "sent" };
    });
    const py = s.schedule({ key: "y" }, async () => {
      order.push("y");
      return { status: "sent" };
    });
    s.cancelQueued();
    release();
    await p0;
    expect((await px).status).toBe("cancelled");
    expect((await py).status).toBe("cancelled");
    expect(order).toEqual(["blocker"]);
    expect(s.pending).toBe(0);
  });

  it("rejects new work after dispose", async () => {
    const s = new CommandScheduler();
    s.dispose();
    const r = await s.schedule({ key: "late" }, async () => ({ status: "sent" }));
    expect(r.status).toBe("cancelled");
  });
});

describe("CommandScheduler error surfacing", () => {
  it("surfaces a thrown transport error as a structured result", async () => {
    const s = new CommandScheduler();
    const r = await s.schedule({ key: "boom" }, async () => {
      throw new Error("gatt failed");
    });
    expect(r.status).toBe("error");
    expect(r.message).toContain("gatt failed");
  });
});

describe("confirmTransition read-back reconciliation", () => {
  function fakeClock() {
    let t = 0;
    return {
      now: () => t,
      sleep: async (ms: number) => {
        t += ms;
      },
    };
  }

  it("confirms immediately when the read-back already matches", async () => {
    const clock = fakeClock();
    const r = await confirmTransition({
      write: async () => {},
      readBack: async () => 42,
      expected: 42,
      timeoutMs: 100,
      pollMs: 10,
      ...clock,
    });
    expect(r.status).toBe("confirmed");
    expect(r.value).toBe(42);
  });

  it("confirms once the device catches up within the window", async () => {
    const clock = fakeClock();
    let calls = 0;
    const r = await confirmTransition({
      write: async () => {},
      readBack: async () => {
        calls += 1;
        return calls >= 3 ? "on" : "off";
      },
      expected: "on",
      timeoutMs: 100,
      pollMs: 10,
      ...clock,
    });
    expect(r.status).toBe("confirmed");
    expect(calls).toBe(3);
  });

  it("reports a mismatch when the device settles on a different value", async () => {
    const clock = fakeClock();
    const r = await confirmTransition({
      write: async () => {},
      readBack: async () => "off",
      expected: "on",
      timeoutMs: 50,
      pollMs: 10,
      ...clock,
    });
    expect(r.status).toBe("mismatch");
    expect(r.expected).toBe("on");
    expect(r.observed).toBe("off");
  });

  it("reports a timeout when no read-back value ever arrives", async () => {
    const clock = fakeClock();
    const r = await confirmTransition({
      write: async () => {},
      readBack: async () => {
        throw new Error("no notification yet");
      },
      expected: "on",
      timeoutMs: 50,
      pollMs: 10,
      ...clock,
    });
    expect(r.status).toBe("timeout");
  });
});
