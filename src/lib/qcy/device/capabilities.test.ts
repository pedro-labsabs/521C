import { describe, expect, it } from "vitest";
import {
  HT08_CAPABILITIES,
  canInteract,
  isExperimentalWrite,
  isImplemented,
  isShown,
  isWritable,
  summarizeCapability,
  type CapabilityTruth,
} from "./capabilities";
import { GENERIC_QCY_PROFILE, HT08_PROFILE } from "./catalog";
import {
  OPCODE_EVIDENCE,
  experimentalWriteOpcodes,
  supportedWriteOpcodes,
} from "../protocol/evidence";

const EVIDENCE = ["supported", "unknown", "unsupported"] as const;
const IMPL = ["implemented", "mock-only", "not-implemented"] as const;
const WRITE = ["writable", "experimental", "read-only", "forbidden"] as const;

function cap(partial: Partial<CapabilityTruth>): CapabilityTruth {
  return {
    hardware: "supported",
    protocol: "supported",
    implementation: "implemented",
    write: "writable",
    ...partial,
  };
}

describe("capability truth structure", () => {
  it("every HT08 capability has valid four-dimensional values", () => {
    for (const [key, c] of Object.entries(HT08_CAPABILITIES)) {
      expect(EVIDENCE, `${key}.hardware`).toContain(c.hardware);
      expect(EVIDENCE, `${key}.protocol`).toContain(c.protocol);
      expect(IMPL, `${key}.implementation`).toContain(c.implementation);
      expect(WRITE, `${key}.write`).toContain(c.write);
    }
  });

  it("covers the full documented HT08 capability surface", () => {
    expect(Object.keys(HT08_CAPABILITIES)).toHaveLength(42);
  });
});

describe("deterministic rules", () => {
  it("isShown hides only features both hardware and protocol reject", () => {
    expect(isShown(cap({}))).toBe(true);
    expect(isShown(cap({ hardware: "unsupported", protocol: "unsupported" }))).toBe(false);
    expect(isShown(cap({ hardware: "unsupported", protocol: "unknown" }))).toBe(true);
    expect(isShown(cap({ hardware: "unknown", protocol: "unsupported" }))).toBe(true);
  });

  it("isWritable requires implementation plus a supported write", () => {
    expect(isWritable(cap({}))).toBe(true);
    expect(isWritable(cap({ write: "experimental" }))).toBe(false);
    expect(isWritable(cap({ write: "read-only" }))).toBe(false);
    expect(isWritable(cap({ implementation: "not-implemented" }))).toBe(false);
    expect(isWritable(cap({ implementation: "mock-only" }))).toBe(false);
  });

  it("isExperimentalWrite flags implemented experimental writes only", () => {
    expect(isExperimentalWrite(cap({ write: "experimental" }))).toBe(true);
    expect(isExperimentalWrite(cap({ write: "writable" }))).toBe(false);
    expect(isExperimentalWrite(cap({ write: "experimental", implementation: "not-implemented" }))).toBe(false);
  });

  it("canInteract offers implemented writable or experimental features", () => {
    expect(canInteract(cap({}))).toBe(true);
    expect(canInteract(cap({ write: "experimental" }))).toBe(true);
    expect(canInteract(cap({ write: "read-only" }))).toBe(false);
    expect(canInteract(cap({ implementation: "not-implemented" }))).toBe(false);
    expect(canInteract(cap({ implementation: "mock-only" }))).toBe(false);
  });

  it("isImplemented distinguishes real, mock and pending", () => {
    expect(isImplemented(cap({}))).toBe(true);
    expect(isImplemented(cap({ implementation: "mock-only" }))).toBe(false);
    expect(isImplemented(cap({ implementation: "not-implemented" }))).toBe(false);
  });
});

describe("honest summaries", () => {
  it("maps each combination to a truthful label", () => {
    expect(summarizeCapability(cap({})).label).toBe("Supported");
    expect(summarizeCapability(cap({ write: "experimental" })).label).toBe("Experimental");
    expect(summarizeCapability(cap({ write: "read-only" })).label).toBe("Read-only");
    expect(summarizeCapability(cap({ implementation: "mock-only" })).label).toBe("Mock only");
    expect(summarizeCapability(cap({ implementation: "not-implemented" })).label).toBe(
      "Protocol known \u00b7 app pending",
    );
    expect(
      summarizeCapability(cap({ implementation: "not-implemented", protocol: "unknown" })).label,
    ).toBe("Needs protocol research");
    expect(
      summarizeCapability(
        cap({ implementation: "not-implemented", hardware: "unsupported", protocol: "unsupported" }),
      ).label,
    ).toBe("Unsupported");
    expect(summarizeCapability(cap({ write: "forbidden" })).tone).toBe("danger");
  });
});

describe("host features are downgraded until implemented (issue #13)", () => {
  it("systemEq is not a writable QCY capability", () => {
    const c = HT08_CAPABILITIES.systemEq;
    expect(c.implementation).toBe("not-implemented");
    expect(c.write).toBe("read-only");
    expect(isWritable(c)).toBe(false);
    expect(canInteract(c)).toBe(false);
  });

  it("autoGameMode is not implemented and writes nothing", () => {
    const c = HT08_CAPABILITIES.autoGameMode;
    expect(c.implementation).toBe("not-implemented");
    expect(c.write).toBe("read-only");
    expect(canInteract(c)).toBe(false);
  });

  it("codecStatus is mock-only, never presented as implemented", () => {
    const c = HT08_CAPABILITIES.codecStatus;
    expect(c.implementation).toBe("mock-only");
    expect(isImplemented(c)).toBe(false);
    expect(canInteract(c)).toBe(false);
  });
});

describe("destructive and unsupported features stay closed", () => {
  it("firmwareOta is forbidden", () => {
    const c = HT08_CAPABILITIES.firmwareOta;
    expect(c.write).toBe("forbidden");
    expect(summarizeCapability(c).tone).toBe("danger");
    expect(isWritable(c)).toBe(false);
  });

  it("findGps is unsupported on HT08", () => {
    const c = HT08_CAPABILITIES.findGps;
    expect(c.hardware).toBe("unsupported");
    expect(c.protocol).toBe("unsupported");
    expect(isShown(c)).toBe(false);
  });
});

describe("consistency with the evidence ledger and write policy", () => {
  const supported = supportedWriteOpcodes();
  const experimental = experimentalWriteOpcodes();

  it("every writable/experimental opcode-backed capability agrees with the ledger", () => {
    for (const [key, c] of Object.entries(HT08_CAPABILITIES)) {
      if (c.opcode === undefined) continue;
      if (c.write !== "writable" && c.write !== "experimental") continue;
      const evidence = OPCODE_EVIDENCE.get(c.opcode);
      expect(evidence, `${key} opcode 0x${c.opcode.toString(16)} must be in the ledger`).toBeDefined();
      if (c.write === "writable") {
        expect(evidence!.trustLevel, `${key} must be write-supported`).toBe("write-supported");
        expect(supported.has(c.opcode), `${key} opcode in supported set`).toBe(true);
      } else {
        expect(evidence!.trustLevel, `${key} must be write-experimental`).toBe("write-experimental");
        expect(experimental.has(c.opcode), `${key} opcode in experimental set`).toBe(true);
      }
    }
  });

  it("writable capability opcodes are authorized by the HT08 write policy", () => {
    for (const [key, c] of Object.entries(HT08_CAPABILITIES)) {
      if (c.opcode === undefined || c.write !== "writable") continue;
      expect(HT08_PROFILE.writePolicy.supportedOpcodes.has(c.opcode), `${key} 0x${c.opcode.toString(16)}`).toBe(true);
    }
  });

  it("experimental capability opcodes require the opt-in policy bucket", () => {
    for (const [key, c] of Object.entries(HT08_CAPABILITIES)) {
      if (c.opcode === undefined || c.write !== "experimental") continue;
      expect(HT08_PROFILE.writePolicy.experimentalOpcodes.has(c.opcode), `${key} 0x${c.opcode.toString(16)}`).toBe(true);
    }
  });

  it("no read-only/forbidden capability is writable through the policy", () => {
    for (const [key, c] of Object.entries(HT08_CAPABILITIES)) {
      if (c.write === "writable" || c.write === "experimental") continue;
      expect(isWritable(c), `${key} must not be writable`).toBe(false);
    }
  });
});

describe("generic/unknown profile stays read-only", () => {
  it("non-read capabilities are unknown and not implemented", () => {
    const c = GENERIC_QCY_PROFILE.capabilities.ancOn;
    expect(c.hardware).toBe("unknown");
    expect(c.protocol).toBe("unknown");
    expect(c.implementation).toBe("not-implemented");
    expect(c.write).toBe("read-only");
    expect(canInteract(c)).toBe(false);
  });

  it("keeps basic read capabilities", () => {
    expect(GENERIC_QCY_PROFILE.capabilities.batteryLeft.implementation).toBe("implemented");
    expect(GENERIC_QCY_PROFILE.capabilities.firmware.implementation).toBe("implemented");
  });

  it("generic profile exposes an empty write policy", () => {
    expect(GENERIC_QCY_PROFILE.writePolicy.supportedOpcodes.size).toBe(0);
    expect(GENERIC_QCY_PROFILE.writePolicy.experimentalOpcodes.size).toBe(0);
    expect(GENERIC_QCY_PROFILE.readOnly).toBe(true);
  });
});
