#!/usr/bin/env python3
"""Serve the built Slidev deck with browser deep-link support.

The Slidev build uses /1, /2, ... as client-side slide routes. Python's
SimpleHTTPServer otherwise treats those paths as files and returns 404.
"""

from __future__ import annotations

import argparse
import re
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from urllib.parse import urlsplit


SLIDE_ROUTE = re.compile(r"^/(?:\d+|presenter(?:/(?:\d+|print))?|overview|notes|print)/?$")


class DemoHandler(SimpleHTTPRequestHandler):
    def _slide_route(self) -> str | None:
        parsed = urlsplit(self.path)
        match = SLIDE_ROUTE.fullmatch(parsed.path)
        if not match:
            return None
        return "/" + (f"?{parsed.query}" if parsed.query else "")

    def _serve(self, method: str) -> None:
        original_path = self.path
        parsed = urlsplit(original_path)
        if SLIDE_ROUTE.fullmatch(parsed.path) and parsed.path.endswith("/"):
            query = f"?{parsed.query}" if parsed.query else ""
            self.send_response(302)
            self.send_header("Location", parsed.path.rstrip("/") + query)
            self.end_headers()
            return
        rewritten_path = self._slide_route()
        if rewritten_path is not None:
            self.path = rewritten_path
        try:
            if method == "HEAD":
                super().do_HEAD()
            else:
                super().do_GET()
        finally:
            self.path = original_path

    def do_GET(self) -> None:  # noqa: N802 - required by http.server
        self._serve("GET")

    def do_HEAD(self) -> None:  # noqa: N802 - required by http.server
        self._serve("HEAD")


def main() -> None:
    parser = argparse.ArgumentParser(description="Serve the Focaus HTML roadshow")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=1421)
    parser.add_argument(
        "--directory",
        type=Path,
        default=Path(__file__).with_name("dist"),
        help="Built HTML directory (defaults to presentation/dist)",
    )
    args = parser.parse_args()

    directory = args.directory.expanduser().resolve()
    if not (directory / "index.html").is_file():
        raise SystemExit(f"找不到演示入口：{directory / 'index.html'}，请先运行 pnpm build")

    handler = partial(DemoHandler, directory=str(directory))
    server = ThreadingHTTPServer((args.host, args.port), handler)
    print(f"Serving {directory}")
    print(f"Open http://{args.host}:{args.port}/ or /1 through /8")
    print("Press Control+C to stop.")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nStopped.")
    finally:
        server.server_close()


if __name__ == "__main__":
    main()
