#!/usr/bin/env bash
# Hardware probe for issue #50: exercise the HT08 SPP control channel.
# Run with the earbuds OUT of the case (ideally connected for A2DP so they stay awake).
# Stage-2-safe by default: read-only probes. Pass WRITE=1 to also send the
# allowlisted, reversible ANC toggle (0x0C) and confirm the effect by ear.
set -u
ADDR="${1:-84:AC:60:62:69:DA}"
WRITE="${WRITE:-0}"

python3 - "$ADDR" "$WRITE" <<'PYEOF'
import socket, struct, sys, time

ADDR, WRITE = sys.argv[1], sys.argv[2] == "1"

def rfcomm_session(label, sends, wait=4.0):
    s = socket.socket(31, socket.SOCK_STREAM, 3)  # AF_BLUETOOTH, BTPROTO_RFCOMM
    s.settimeout(20)
    try:
        s.connect((ADDR, 1))
    except OSError as e:
        print(f"[{label}] CONNECT_FAIL: {e}")
        return
    s.settimeout(0.4)
    for name, data in sends:
        if data:
            s.sendall(data)
            print(f"[{label}] tx[{name}]: {data.hex()}")
        end = time.time() + wait
        while time.time() < end:
            try:
                c = s.recv(256)
                if not c:
                    print(f"[{label}] EOF"); break
                print(f"[{label}] rx[{name}] ({len(c)}): {c.hex()}")
            except socket.timeout:
                continue
            except OSError as e:
                print(f"[{label}] rx err: {e}"); break
    s.close()
    time.sleep(1)

# 1. battery read attempt (RequestData 0xFE/0x2F), long wait for any data frame
rfcomm_session("battery", [("req", bytes.fromhex("ff03fe012f"))], wait=6)
# 2. version read attempt
rfcomm_session("version", [("req", bytes.fromhex("ff03fe0130"))], wait=6)
# 3. well-formed RCSP get-device-info (FE DC BA framing, opcode 0x01)
rfcomm_session("rcsp-info", [("req", bytes.fromhex("fedcba01c0000101ef"))], wait=4)

if WRITE:
    # 4. allowlisted reversible write: ANC on (0x0C 0x01), then off (0x0C 0x00)
    rfcomm_session("anc-on", [("req", bytes.fromhex("ff030c0101"))], wait=3)
    print(">>> confirm ANC engaged by ear, then off in 5 s")
    time.sleep(5)
    rfcomm_session("anc-off", [("req", bytes.fromhex("ff030c0100"))], wait=3)
    print(">>> confirm ANC off")
else:
    print("WRITE=0: skipping ANC write probe (set WRITE=1 to include it)")
PYEOF
