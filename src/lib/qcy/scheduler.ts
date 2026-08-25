/**
 * BLE command scheduling, coalescing and confirmed state transitions (issue #10).
 *
 * A small, deterministic, event-driven scheduler suited to low-overhead BLE control:
 *
 *   - Serializes device writes per active connection (one command runs at a time).
 *   - Coalesces high-frequency latest-value controls (EQ sliders, balance, ANC level):
 *     a queued command with the same key is superseded by the newest value instead of
 *     accumulating an unbounded backlog.
 *   - Preserves FIFO ordering for commands that form a logical sequence.
 *   - Distinguishes transport-write completion ("sent") from confirmed device state
 *     ("confirmed") via an optional bounded read-back reconciliation.
 *   - Surfaces timeout / rejection / mismatch / cancellation as structured results.
 *
 * The scheduler is event-driven: it only pumps when work is enqueued and never runs a
 * permanent high-frequency polling loop.
 */

export type CommandStatus =
  | "sent"
  | "confirmed"
  | "coalesced"
  | "cancelled"
  | "denied"
  | "timeout"
  | "mismatch"
  | "error";

export type CommandResult = {
  status: CommandStatus;
  message?: string;
  value?: unknown;
  expected?: unknown;
  observed?: unknown;
};

export type CommandOptions = {
  /** Stable identity used for coalescing and ordering. */
  key: string;
  /** Latest-value command: a queued command with the same key is superseded. */
  coalesce?: boolean;
};

type QueueItem = {
  key: string;
  coalesce: boolean;
  run: () => Promise<CommandResult>;
  resolve: (r: CommandResult) => void;
};

export class CommandScheduler {
  private queue: QueueItem[] = [];
  private pumping = false;
  private disposed = false;

  /** Number of commands queued but not yet started. */
  get pending(): number {
    return this.queue.length;
  }

  /**
   * Enqueue a command. Resolves with a structured result once the command runs, is
   * superseded by a newer coalesced command, or is cancelled.
   */
  schedule(opts: CommandOptions, run: () => Promise<CommandResult>): Promise<CommandResult> {
    return new Promise((resolve) => {
      if (this.disposed) {
        resolve({ status: "cancelled", message: "scheduler disposed" });
        return;
      }
      const item: QueueItem = { key: opts.key, coalesce: opts.coalesce ?? false, run, resolve };
      if (item.coalesce) {
        const idx = this.queue.findIndex((q) => q.coalesce && q.key === item.key);
        if (idx !== -1) {
          const superseded = this.queue[idx]!;
          this.queue[idx] = item;
          superseded.resolve({ status: "coalesced", message: `superseded by newer "${item.key}"` });
          return;
        }
      }
      this.queue.push(item);
      void this.pump();
    });
  }

  private async pump(): Promise<void> {
    if (this.pumping || this.disposed) return;
    this.pumping = true;
    try {
      while (this.queue.length > 0 && !this.disposed) {
        const item = this.queue.shift()!;
        let result: CommandResult;
        try {
          result = await item.run();
        } catch (err) {
          result = { status: "error", message: err instanceof Error ? err.message : String(err) };
        }
        item.resolve(result);
      }
    } finally {
      this.pumping = false;
    }
  }

  /** Cancel all queued (not yet running) work, e.g. on connection loss. */
  cancelQueued(reason = "connection lost"): void {
    const pending = this.queue.splice(0, this.queue.length);
    for (const item of pending) item.resolve({ status: "cancelled", message: reason });
  }

  /** Cancel queued work and reject anything enqueued afterwards. */
  dispose(): void {
    this.disposed = true;
    this.cancelQueued("scheduler disposed");
  }
}

const defaultSleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/**
 * Write and then reconcile the requested value against the observed value using a
 * bounded read-back window. Returns "confirmed" on match, "mismatch" when the device
 * reports a definite different value, or "timeout" when no confirmation arrives.
 *
 * `now`/`sleep` are injectable so tests stay deterministic and no real timer is needed.
 */
export async function confirmTransition(opts: {
  write: () => Promise<void>;
  readBack: () => Promise<unknown>;
  expected: unknown;
  equals?: (a: unknown, b: unknown) => boolean;
  timeoutMs?: number;
  pollMs?: number;
  now?: () => number;
  sleep?: (ms: number) => Promise<void>;
}): Promise<CommandResult> {
  const now = opts.now ?? Date.now;
  const sleep = opts.sleep ?? defaultSleep;
  const equals = opts.equals ?? Object.is;
  const pollMs = opts.pollMs ?? 50;
  const deadline = now() + (opts.timeoutMs ?? 1500);

  await opts.write();

  let observed: unknown;
  try {
    observed = await opts.readBack();
  } catch {
    observed = undefined;
  }
  if (equals(opts.expected, observed)) return { status: "confirmed", value: observed };

  while (now() < deadline) {
    await sleep(pollMs);
    try {
      observed = await opts.readBack();
    } catch {
      observed = undefined;
    }
    if (equals(opts.expected, observed)) return { status: "confirmed", value: observed };
  }

  return observed === undefined
    ? { status: "timeout", expected: opts.expected }
    : { status: "mismatch", expected: opts.expected, observed };
}
