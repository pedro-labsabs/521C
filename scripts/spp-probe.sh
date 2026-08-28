#!/usr/bin/env bash
# Generic SPP/RFCOMM probe (issue #50 history, retargeted after #52).
#
# NOT an HT08 control probe: live validation (#52) proved the HT08 control
# path is BLE GATT on the separate LE identity, and HT08 SPP channel 1
# ("COM5") only byte-ACKs frames without executing them. Use this script to
# characterize SPP behavior of OTHER QCY models whose evidence may point at
# RFCOMM, or to re-confirm the HT08 byte-ACK behavior.
#
# Read-only by default. WRITE=1 sends one validated 0x17 ANC scene frame
# (indoor scene) — only meaningful on a model whose evidence shows SPP
# command execution; on HT08 it demonstrates the byte-ACK-only behavior.
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
    # 4. validated 0x17 ANC scene write (indoor (1,1,2)). 0x0C is falsified
    # on HT08 and never probed. On HT08 expect only the byte-ACK bridge
    # response and NO audible effect — that is the documented result.
    rfcomm_session("anc-scene-indoor", [("req", bytes.fromhex("ff051703010102"))], wait=3)
    print(">>> on a model with evidenced SPP execution, confirm ANC indoor by ear")
else:
    print("WRITE=0: skipping write probe (set WRITE=1 to include it)")
PYEOF
