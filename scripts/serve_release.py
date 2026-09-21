#!/usr/bin/env python3
"""Static file server with SPA fallback, for exercising a release bundle locally.

`dx serve` is the development server; this exists so the *release* artefact can be
verified as it would actually be deployed — no dev websocket, no rebuild overlay.
Any path that is not a real file falls back to index.html, which is what client-side
routing needs from a host.
"""

import http.server
import os
import sys

ROOT = sys.argv[1] if len(sys.argv) > 1 else "."
PORT = int(sys.argv[2]) if len(sys.argv) > 2 else 8081


class SpaHandler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=ROOT, **kwargs)

    def send_response(self, *args, **kwargs):
        super().send_response(*args, **kwargs)
        # A stale wasm bundle is indistinguishable from a broken one; never cache here.
        self.send_header("Cache-Control", "no-store")

    def do_GET(self):
        path = self.translate_path(self.path)
        if not os.path.isfile(path) and not os.path.isdir(path):
            self.path = "/index.html"
        return super().do_GET()

    def log_message(self, fmt, *args):
        sys.stderr.write("%s %s\n" % (self.address_string(), fmt % args))


if __name__ == "__main__":
    http.server.ThreadingHTTPServer(("127.0.0.1", PORT), SpaHandler).serve_forever()
