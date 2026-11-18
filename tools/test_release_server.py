#!/usr/bin/env python3
"""A local release server for manual tests of the launcher (docs/release-api.md), loopback only.

Usage: tools/test_release_server.py MANIFEST.json [--port 8765]
MANIFEST.json is a list of {"product": "game|launcher", "version": "0.2.0", "platform": "windows-x86_64",
"channel": "stable", "zip": "path.zip", "sig": "valid|missing|wrongkey", "notes": "text"} (sig and notes optional).
Signs with the public TEST key (seed 00 01 .. 1f, crates/core/src/signature/tests.rs), so run the launcher with a
DEBUG-ASSERTIONS build and CASTANEA_TRUST_KEYS_FILE naming a file with its public key (print it with --pubkey).
Serves latest, the list and the download with Range. Nothing is published anywhere; it binds 127.0.0.1 only.
"""
import base64, hashlib, json, os, re, sys
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import parse_qs, urlparse

from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

TEST = Ed25519PrivateKey.from_private_bytes(bytes(range(32)))
OTHER = Ed25519PrivateKey.from_private_bytes(bytes(range(1, 33)))


def raw_public(key):
    return key.public_key().public_bytes(Encoding.Raw, PublicFormat.Raw)


def key_id(key):
    return hashlib.sha256(raw_public(key)).digest()[:8].hex()


def semver(v):
    core, _, pre = v.partition("-")
    return (tuple(int(x) for x in core.split(".")), pre == "", pre)


class Server(ThreadingHTTPServer):
    def __init__(self, addr, manifest):
        super().__init__(addr, Handler)
        self.base = f"http://127.0.0.1:{addr[1]}"
        self.entries = []
        for item in manifest:
            data = open(item["zip"], "rb").read()
            sha = hashlib.sha256(data).hexdigest()
            self.entries.append({**item, "data": data, "sha": sha})

    def body(self, e):
        sig, kid = None, None
        mode = e.get("sig", "valid")
        if mode != "missing":
            key = OTHER if mode == "wrongkey" else TEST
            text = "\n".join(["castanea-release-v1", e["product"], e["version"], e["platform"], e["channel"], e["sha"], str(len(e["data"]))])
            sig, kid = base64.b64encode(key.sign(text.encode())).decode(), key_id(key)
        suffix = "?product=launcher" if e["product"] == "launcher" else ""
        return {
            "product": e["product"], "version": e["version"], "channel": e["channel"], "platform": e["platform"],
            "released_at": "2026-11-18T00:00:00Z", "size": len(e["data"]), "sha256": e["sha"],
            "url": f"{self.base}/v1/releases/{e['version']}/{e['platform']}/download{suffix}",
            "notes": e.get("notes", "Local test release."), "signature": sig, "key_id": kid,
        }


class Handler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        sys.stderr.write("test-server: %s %s\n" % (self.command, self.path))

    def send_json(self, code, obj):
        raw = json.dumps(obj).encode()
        self.send_response(code)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(raw)))
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        url = urlparse(self.path)
        q = {k: v[0] for k, v in parse_qs(url.query).items()}
        product, platform, channel = q.get("product", "game"), q.get("platform", ""), q.get("channel", "stable")
        pool = [e for e in self.server.entries if e["product"] == product and e["platform"] == platform and e["channel"] == channel]
        if url.path == "/v1/releases/latest":
            if not pool:
                return self.send_json(404, {"error": "no release"})
            return self.send_json(200, self.server.body(max(pool, key=lambda e: semver(e["version"]))))
        if url.path == "/v1/releases":
            pool.sort(key=lambda e: semver(e["version"]), reverse=True)
            return self.send_json(200, {"releases": [self.server.body(e) for e in pool]})
        m = re.fullmatch(r"/v1/releases/([^/]+)/([^/]+)/download", url.path)
        if m:
            found = [e for e in self.server.entries if e["product"] == product and e["version"] == m[1] and e["platform"] == m[2]]
            if not found:
                return self.send_json(404, {"error": "no release"})
            return self.download(found[0])
        self.send_json(404, {"error": "not found"})

    def download(self, e):
        data, start, code = e["data"], 0, 200
        etag = f'"{e["sha"]}"'
        rng = self.headers.get("Range")
        if rng and self.headers.get("If-Range", etag) == etag and (r := re.fullmatch(r"bytes=(\d+)-", rng)):
            start, code = int(r[1]), 206
        part = data[start:]
        self.send_response(code)
        self.send_header("Content-Type", "application/zip")
        self.send_header("Content-Length", str(len(part)))
        self.send_header("ETag", etag)
        if code == 206:
            self.send_header("Content-Range", f"bytes {start}-{len(data) - 1}/{len(data)}")
        self.end_headers()
        self.wfile.write(part)


def main():
    if "--pubkey" in sys.argv:
        print(base64.b64encode(raw_public(TEST)).decode())
        return
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    port = int(sys.argv[sys.argv.index("--port") + 1]) if "--port" in sys.argv else 8765
    if "--port" in sys.argv:
        args.remove(str(port))
    if len(args) != 1:
        sys.exit(__doc__)
    manifest = json.load(open(args[0]))
    base_dir = os.path.dirname(os.path.abspath(args[0]))
    for item in manifest:
        item["zip"] = os.path.join(base_dir, item["zip"])
    srv = Server(("127.0.0.1", port), manifest)
    print(f"test-server: {srv.base} with {len(manifest)} releases", flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main()
