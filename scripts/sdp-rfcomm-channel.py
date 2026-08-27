#!/usr/bin/env python3
"""Read-only SDP query: resolve the RFCOMM channel of an SPP service.

521C hardware-validation tooling (issue #50, Stage 1 — read-only discovery).
Sends a single SDP ServiceSearchAttributeRequest (PDU 0x06) over L2CAP PSM 1
for a service UUID (default: SPP 0x00001101) and prints the RFCOMM channel
from the ProtocolDescriptorList (attribute 0x0004). It never writes anything
to the device and never mutates host state; the device must be awake/out of
the case (BR/EDR pageable), otherwise connect fails with EHOSTDOWN.

Usage: python3 scripts/sdp-rfcomm-channel.py <AA:BB:CC:DD:EE:FF> [uuid16-hex]
"""
import socket
import struct
import sys

AF_BLUETOOTH = 31
BTPROTO_L2CAP = 0
SDP_PSM = 1
SERVICE_SEARCH_ATTRIBUTE_REQUEST = 0x06
SERVICE_SEARCH_ATTRIBUTE_RESPONSE = 0x07
ATTR_PROTOCOL_DESCRIPTOR_LIST = 0x0004
ATTR_SERVICE_NAME = 0x0100
UUID_RFCOMM = 0x0003


def de_uuid16(u: int) -> bytes:
    return b"\x19" + struct.pack(">H", u)


def de_seq(payload: bytes) -> bytes:
    n = len(payload)
    if n < 256:
        return b"\x35" + bytes([n]) + payload
    return b"\x37" + struct.pack(">H", n) + payload


def de_range32(lo: int, hi: int) -> bytes:
    return b"\x0a" + struct.pack(">II", lo, hi)


def parse(buf: bytes, i: int = 0):
    """Minimal SDP data-element parser. Returns (value, next_index)."""
    h = buf[i]
    etype = h >> 3
    size_desc = h & 7
    i += 1
    if size_desc == 5:
        length = 0
    elif size_desc <= 4:
        length = [1, 1, 2, 4, 8, 16][size_desc]
    elif size_desc == 6:
        length = buf[i]
        i += 1
    elif size_desc == 7:
        length = struct.unpack(">H", buf[i:i + 2])[0]
        i += 2
    else:
        raise ValueError(f"bad size descriptor {size_desc:#x}")
    raw = buf[i:i + length]
    i += length
    if etype in (2, 3):  # dataseq / altseq
        kids, j = [], 0
        while j < len(raw):
            v, j = parse(raw, j)
            kids.append(v)
        return (etype, kids), i
    if etype == 1:  # uuid
        if length == 2:
            return ("uuid16", struct.unpack(">H", raw)[0]), i
        if length == 4:
            return ("uuid32", struct.unpack(">I", raw)[0]), i
        return ("uuid128", raw.hex()), i
    if etype == 0:  # unsigned int
        return ("uint", int.from_bytes(raw, "big")), i
    if etype == 4:  # text
        return ("text", raw.decode("utf-8", "replace")), i
    return (f"type{etype}", raw.hex()), i


def main() -> int:
    if len(sys.argv) < 2:
        print(__doc__)
        return 1
    bdaddr = sys.argv[1]
    uuid16 = int(sys.argv[2], 16) if len(sys.argv) > 2 else 0x1101

    params = (
        de_seq(de_uuid16(uuid16))
        + struct.pack(">H", 0xFFFF)
        + de_seq(de_range32(0x0000, 0xFFFF))
    )
    pdu = (
        bytes([SERVICE_SEARCH_ATTRIBUTE_REQUEST])
        + struct.pack(">HH", 1, len(params))
        + params
    )

    sock = socket.socket(AF_BLUETOOTH, socket.SOCK_STREAM, BTPROTO_L2CAP)
    sock.settimeout(15)
    try:
        sock.connect((bdaddr, SDP_PSM))
    except OSError as e:
        print(f"CONNECT_FAIL: {e} (device asleep/out of range?)")
        return 2
    sock.sendall(pdu)
    hdr = sock.recv(5)
    if len(hdr) < 5 or hdr[0] != SERVICE_SEARCH_ATTRIBUTE_RESPONSE:
        print(f"BAD_RESPONSE: {hdr.hex()}")
        return 3
    plen = struct.unpack(">H", hdr[3:5])[0]
    buf = b""
    while len(buf) < plen:
        chunk = sock.recv(plen - len(buf))
        if not chunk:
            break
        buf += chunk
    sock.close()

    count = struct.unpack(">H", buf[:2])[0]
    top, _ = parse(buf[2:2 + count])
    records = top[1] if top[0][0] == 2 else []
    print(f"RECORDS_FOUND: {len(records)}")
    found_channel = None
    for rec in records:
        attrs = {}
        for aid_el, val_el in rec[1]:
            if isinstance(aid_el[1], int):
                attrs[aid_el[1]] = val_el
        name = attrs.get(ATTR_SERVICE_NAME)
        print(f"  service name: {name[1] if name else '?'}")
        pdl = attrs.get(ATTR_PROTOCOL_DESCRIPTOR_LIST)
        if not pdl or pdl[0][0] != 2:
            continue
        for layer in pdl[1]:
            parts = [p[1] for p in layer[1]]
            print(f"    protocol layer: {parts}")
            if layer[1] and layer[1][0][1] == ("uuid16", UUID_RFCOMM):
                for p in layer[1][1:]:
                    if p[0] == "uint":
                        found_channel = p[1]
                        print(f"    >>> RFCOMM CHANNEL = {p[1]}")
    if found_channel is None:
        print("NO_RFCOMM_CHANNEL_FOUND")
        return 4
    return 0


if __name__ == "__main__":
    sys.exit(main())
