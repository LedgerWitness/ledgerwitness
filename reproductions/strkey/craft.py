"""Strkey cases implied by SEP-23's rules but absent from its vector list.
Encoder written from SEP-23 §Specification, independent of every SDK."""
import base64, json, struct

def crc16(data):  # CRC-16/XMODEM: poly 0x1021, init 0
    crc = 0
    for b in data:
        crc ^= b << 8
        for _ in range(8):
            crc = ((crc << 1) ^ 0x1021) if crc & 0x8000 else (crc << 1)
            crc &= 0xFFFF
    return crc

def encode(version, body):
    raw = bytes([version]) + body
    raw += struct.pack("<H", crc16(raw))
    return base64.b32encode(raw).decode().rstrip("=")

ALPH = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567"
def set_unused_bits(s):
    """Same string with the unused low bits of the last symbol set to 1."""
    nbits = len(s) * 5
    nbytes = nbits // 8
    unused = nbits - nbytes * 8
    if unused == 0:
        return None
    v = ALPH.index(s[-1]) | ((1 << unused) - 1)
    return s[:-1] + ALPH[v]

key = bytes(range(32))
P = 15 << 3
G = 6 << 3
out = []
def signed(payload, declared=None):
    n = len(payload) if declared is None else declared
    pad = b"\0" * ((4 - len(payload) % 4) % 4)
    return key + struct.pack(">I", n) + payload + pad

for n in range(1, 65):
    good = encode(P, signed(bytes([0xAB]) * n))
    bad = set_unused_bits(good)
    if bad:
        out.append({"expect": "invalid", "desc": f"P with a {n}-byte payload and non-zero unused trailing bits (SEP-23: unused bits must be zero)", "strkey": bad})
        out.append({"expect": "valid", "desc": f"control: the canonical P with a {n}-byte payload", "strkey": good})
valid_g = encode(G, key)
valid_p = encode(P, signed(b"\x01\x02\x03"))
out += [
    {"expect": "invalid", "desc": "lowercase G (re-encodes as uppercase, so fails SEP-23's round-trip rule)", "strkey": valid_g.lower()},
    {"expect": "invalid", "desc": "lowercase P", "strkey": valid_p.lower()},
    {"expect": "invalid", "desc": "G with base-32 '=' padding (SEP-23: no padding)", "strkey": valid_g + "===="},
    {"expect": "invalid", "desc": "P with a 65-byte payload (over the XDR opaque<64> limit)", "strkey": encode(P, signed(b"\xab" * 65))},
    {"expect": "unspecified", "desc": "P with an empty payload (core rejects it as a signer; SEP-23 does not say)", "strkey": encode(P, signed(b""))},
    {"expect": "valid", "desc": "control: canonical G", "strkey": valid_g},
]
json.dump(out, open(__import__("sys").argv[1], "w"), indent=1)  # merged into vectors.json
print(len(out), "crafted cases;", sum(o["expect"] == "invalid" for o in out), "invalid")
