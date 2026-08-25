import { Cmd, DESTRUCTIVE_CMDS, decodePacket } from "./protocol";
import type { QcyDeviceProfile } from "./device/catalog";

/**
 * Central BLE write-authorization policy.
 *
 * This module is the single enforcement point that decides whether an outbound
 * BLE write is allowed for the connected device/profile. It is owned below the
 * UI layer; every transport write path (UI action, profile automation, CLI,
 * future native bridge, or a raw frame constructed in a test) must pass through
 * it. See docs/SECURITY_MODEL.md.
 *
 * The policy is pure (no I/O) so it can be tested directly, and it is also
 * invoked from inside each transport's write/writeDirect so it cannot be
 * bypassed by reaching for a lower-level transport call.
 */

/** Structured denial reasons suitable for UI/CLI reporting. */
export type WriteDenialCode =
  | "destructive-opcode"
  | "device-read-only"
  | "opcode-not-writable"
  | "experimental-opt-in-required"
  | "direct-char-not-allowed"
  | "undecodable-frame";

export type WriteDenial = {
  code: WriteDenialCode;
  message: string;
  opcode?: number;
};

export type WriteAuth = { ok: true } | { ok: false; denial: WriteDenial };

/** Session-scoped opt-in state. Never persisted across sessions. */
export type SessionOptIn = {
  /** Enables writes that are marked experimental for the connected profile. */
  experimental: boolean;
};

export const DEFAULT_OPT_IN: SessionOptIn = { experimental: false };

/** Error thrown by enforced transport writes when the policy denies them. */
export class WriteDeniedError extends Error {
  readonly denial: WriteDenial;
  constructor(denial: WriteDenial) {
    super(denial.message);
    this.name = "WriteDeniedError";
    this.denial = denial;
  }
}

function deny(code: WriteDenialCode, message: string, opcode?: number): WriteAuth {
  return { ok: false, denial: { code, message, opcode } };
}

/** Byte that disables an enable/disable-style command (see enableByte). */
const DISABLE_BYTE = 0x02;

function isPureDisable(params: Uint8Array): boolean {
  return params.length === 1 && params[0] === DISABLE_BYTE;
}

/**
 * Authorize a framed write (one or more command blocks written to the command
 * characteristic). Every block must be allowed for the write to proceed.
 */
export function authorizeFrameWrite(
  profile: QcyDeviceProfile,
  optIn: SessionOptIn,
  bytes: Uint8Array,
): WriteAuth {
  const decoded = decodePacket(bytes);
  if (!decoded.ok) {
    return deny(
      "undecodable-frame",
      `Refusing to write an undecodable frame: ${decoded.error.message}`,
    );
  }

  const policy = profile.writePolicy;

  for (const block of decoded.packet.blocks) {
    const cmd = block.cmd;

    // Destructive opcodes are rejected at this boundary regardless of caller,
    // profile, or opt-in. They are never reachable from unattended automation.
    if (DESTRUCTIVE_CMDS.has(cmd)) {
      return deny(
        "destructive-opcode",
        `Destructive opcode 0x${cmd.toString(16).padStart(2, "0")} is never written by 521C.`,
        cmd,
      );
    }

    // RequestData (0xFE) is a read-back request, not a state mutation. It is
    // allowed even for read-only profiles so status/identification can be read.
    if (cmd === Cmd.RequestData) {
      continue;
    }

    // Unknown/generic devices are read-only by default: no state-changing writes.
    if (profile.readOnly) {
      return deny(
        "device-read-only",
        `${profile.title} is read-only until the model is identified.`,
        cmd,
      );
    }

    if (policy.supportedOpcodes.has(cmd)) {
      continue;
    }

    if (policy.experimentalOpcodes.has(cmd)) {
      // Disabling an experimental feature is always safe; enabling it requires
      // an explicit session opt-in.
      if (isPureDisable(block.params)) {
        continue;
      }
      if (optIn.experimental) {
        continue;
      }
      return deny(
        "experimental-opt-in-required",
        `Opcode 0x${cmd.toString(16).padStart(2, "0")} is experimental for ${profile.title}. Enable the session experimental opt-in first.`,
        cmd,
      );
    }

    return deny(
      "opcode-not-writable",
      `Opcode 0x${cmd.toString(16).padStart(2, "0")} is not a writable command for ${profile.title}.`,
      cmd,
    );
  }

  return { ok: true };
}

/**
 * Authorize a direct (unframed) write to a specific characteristic. Only
 * allowlisted characteristics associated with the connected profile may be
 * written.
 */
export function authorizeDirectWrite(
  profile: QcyDeviceProfile,
  optIn: SessionOptIn,
  charUuid: string,
  _bytes: Uint8Array,
): WriteAuth {
  const normalized = charUuid.toLowerCase();

  if (profile.readOnly) {
    return deny(
      "device-read-only",
      `${profile.title} is read-only until the model is identified.`,
    );
  }

  if (!profile.writePolicy.directChars.has(normalized)) {
    return deny(
      "direct-char-not-allowed",
      `Direct writes to characteristic ${normalized} are not allowed for ${profile.title}.`,
    );
  }

  // Direct-write characteristics in the current HT08 policy are all supported
  // (touch mapping, device EQ). If an experimental direct characteristic is ever
  // added, gate it on optIn.experimental here.
  void optIn;
  return { ok: true };
}
