#!/usr/bin/env python3
"""Screenshot and audit the Fidryn site and the mill in headless Chrome.

Standard library only (Python 3.12+):

    uv run python .agents/scripts/shoot.py serve [--port 8752]
    uv run python .agents/scripts/shoot.py site --out DIR [--widths 390,820,1440]
                                                          [--themes light,dark]
    uv run python .agents/scripts/shoot.py mill --url http://127.0.0.1:8751 --out DIR

`site` serves `site/` with clean URLs, screenshots every page at each width
and theme, fails when a page scrolls sideways or throws a JavaScript error,
and runs keyboard, drawer, theme, and specimen checks. `mill` screenshots a
running `fidryn ui` in its main states and fails on JavaScript errors or
sideways scrolling. Both press Tab round every page (the mill's after its
last state, with a result and history on it) and fail on a focus ring that
is missing or cut off, or a focused control that is out of view or covered.
The mill also fails when its page has no skip link. JavaScript errors include
`console.error` calls. Chrome runs with a throwaway profile, background
downloads turned off, and every host except 127.0.0.1 and localhost blocked;
the profile is deleted when the run ends, Ctrl-C and SIGTERM included.
Set `CHROME` to another Chrome or Chromium binary on other systems.
"""

from __future__ import annotations

import argparse
import base64
import functools
import http.server
import json
import os
import shutil
import signal
import socket
import struct
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from collections.abc import Callable, Iterator
from contextlib import contextmanager
from dataclasses import dataclass, field
from pathlib import Path
from types import FrameType
from typing import Any, NoReturn

REPO = Path(__file__).resolve().parents[2]
CHROME = os.environ.get("CHROME", "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome")
CHROME_FLAGS = [
    "--headless=new",
    "--disable-gpu",
    "--hide-scrollbars",
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-component-update",
    "--disable-background-networking",
    "--disable-sync",
    "--disable-default-apps",
    "--disable-extensions",
    "--disable-features=OptimizationGuideModelDownloading,OptimizationHintsFetching,"
    "OptimizationTargetPrediction,OptimizationHints,MediaRouter",
    # Chrome still calls Google (updates, GCM, autofill, time) with the flags
    # above; resolving no host but 127.0.0.1 and localhost keeps it off the
    # network.
    "--host-resolver-rules=MAP * ~NOTFOUND , EXCLUDE 127.0.0.1, EXCLUDE localhost",
]
SITE_PAGES = [
    "/",
    "/docs/",
    "/docs/getting-started",
    "/docs/language",
    "/docs/cases-and-time",
    "/docs/cli",
    "/docs/mill",
    "/docs/outcomes",
    "/docs/examples",
    "/docs/contributing",
    "/no-such-page",
]
# Line 7 is a declaration the parser rejects (E100 unknown declaration `colour`).
# It sits at module level: inside a query body the parser reads each word as an
# expression statement and reports nothing.
BAD_MODULE = (
    'module Examples.T version "0.1.0" {\n'
    "    jurisdiction Test\n"
    "    effective_at 2026-09-17\n"
    "    outside_scope { complete_instruments }\n"
    "\n"
    "    query q() -> Bool { goal Evaluate { true } }\n"
    "    colour blue\n"
    "}\n"
)


@dataclass(frozen=True)
class Ok[T]:
    value: T


@dataclass(frozen=True)
class Err[E]:
    error: E


type Result[T, E] = Ok[T] | Err[E]


@dataclass
class Report:
    """What a run found: screenshots written and failures to act on."""

    shots: list[Path] = field(default_factory=list)
    failures: list[str] = field(default_factory=list)


class WebSocket:
    """A minimal RFC 6455 client: masked text frames out, fragmented frames in."""

    def __init__(self, sock: socket.socket) -> None:
        self._sock = sock
        self._buf = bytearray()

    @classmethod
    def connect(cls, url: str, timeout: float = 60.0) -> WebSocket:
        hostport, _, path = url.removeprefix("ws://").partition("/")
        host, _, port = hostport.partition(":")
        sock = socket.create_connection((host, int(port)), timeout=timeout)
        try:
            key = base64.b64encode(os.urandom(16)).decode()
            sock.sendall(
                (
                    f"GET /{path} HTTP/1.1\r\nHost: {hostport}\r\nUpgrade: websocket\r\n"
                    f"Connection: Upgrade\r\nSec-WebSocket-Key: {key}\r\n"
                    "Sec-WebSocket-Version: 13\r\n\r\n"
                ).encode()
            )
            ws = cls(sock)
            status = ws._read_until(b"\r\n\r\n").split(b"\r\n", 1)[0]
            if b" 101 " not in status:
                raise ConnectionError(f"websocket handshake refused: {status!r}")
        except BaseException:
            sock.close()
            raise
        return ws

    def close(self) -> None:
        self._sock.close()

    def send_text(self, text: str) -> None:
        self._send_frame(0x1, text.encode())

    def recv_text(self) -> str:
        parts = bytearray()
        while True:
            b0, b1 = self._read_exact(2)
            fin, opcode, size = b0 & 0x80, b0 & 0x0F, b1 & 0x7F
            if size == 126:
                (size,) = struct.unpack("!H", self._read_exact(2))
            elif size == 127:
                (size,) = struct.unpack("!Q", self._read_exact(8))
            mask = self._read_exact(4) if b1 & 0x80 else b""
            payload = self._read_exact(size)
            if mask:
                payload = bytes(b ^ mask[i % 4] for i, b in enumerate(payload))
            if opcode == 0x8:
                raise ConnectionError("websocket closed by Chrome")
            if opcode == 0x9:
                self._send_frame(0xA, payload)
                continue
            if opcode in (0x0, 0x1, 0x2):
                parts += payload
                if fin:
                    return parts.decode()

    def _send_frame(self, opcode: int, payload: bytes) -> None:
        header = bytearray([0x80 | opcode])
        size = len(payload)
        if size < 126:
            header.append(0x80 | size)
        elif size < 65536:
            header.append(0x80 | 126)
            header += struct.pack("!H", size)
        else:
            header.append(0x80 | 127)
            header += struct.pack("!Q", size)
        mask = os.urandom(4)
        header += mask
        self._sock.sendall(bytes(header) + bytes(b ^ mask[i % 4] for i, b in enumerate(payload)))

    def _read_exact(self, size: int) -> bytes:
        while len(self._buf) < size:
            chunk = self._sock.recv(max(65536, size - len(self._buf)))
            if not chunk:
                raise ConnectionError("websocket closed")
            self._buf += chunk
        data = bytes(self._buf[:size])
        del self._buf[:size]
        return data

    def _read_until(self, marker: bytes) -> bytes:
        while marker not in self._buf:
            chunk = self._sock.recv(65536)
            if not chunk:
                raise ConnectionError("websocket closed during the handshake")
            self._buf += chunk
        end = self._buf.index(marker)
        head = bytes(self._buf[:end])
        del self._buf[: end + len(marker)]
        return head


class Cdp:
    """One Chrome DevTools Protocol session on a page target."""

    def __init__(self, ws: WebSocket) -> None:
        self._ws = ws
        self._next_id = 0
        self.events: list[dict[str, Any]] = []

    def call(self, method: str, **params: object) -> dict[str, Any]:
        self._next_id += 1
        msg_id = self._next_id
        self._ws.send_text(json.dumps({"id": msg_id, "method": method, "params": params}))
        while True:
            msg: dict[str, Any] = json.loads(self._ws.recv_text())
            if msg.get("id") != msg_id:
                self.events.append(msg)
                continue
            if "error" in msg:
                raise RuntimeError(f"{method}: {msg['error']}")
            result: dict[str, Any] = msg.get("result", {})
            return result

    def run_js(self, expression: str, *, await_promise: bool = False) -> Any:
        """Evaluate one of this script's own fixed expressions in the page."""
        res = self.call(
            "Runtime.evaluate",
            expression=expression,
            returnByValue=True,
            awaitPromise=await_promise,
        )
        if "exceptionDetails" in res:
            detail = res["exceptionDetails"].get("exception", {}).get("description", "")
            raise RuntimeError(f"page script failed: {expression[:70]!r}: {detail}")
        return res.get("result", {}).get("value")

    def key(self, key: str, code: str, key_code: int, text: str = "") -> None:
        down: dict[str, object] = {
            "type": "keyDown",
            "key": key,
            "code": code,
            "windowsVirtualKeyCode": key_code,
        }
        if text:
            down["text"] = text
        self.call("Input.dispatchKeyEvent", **down)
        self.call(
            "Input.dispatchKeyEvent",
            type="keyUp",
            key=key,
            code=code,
            windowsVirtualKeyCode=key_code,
        )

    def take_errors(self) -> list[str]:
        """Uncaught exceptions and `console.error` calls seen since the last take.

        The site script catches a failing feature and logs it with
        `console.error`, so uncaught exceptions alone would miss it.
        """
        errors: list[str] = []
        for event in self.events:
            method = event.get("method")
            params: dict[str, Any] = event.get("params", {})
            if method == "Runtime.exceptionThrown":
                errors.append(
                    str(params["exceptionDetails"].get("exception", {}).get("description", "error"))
                )
            elif method == "Runtime.consoleAPICalled" and params.get("type") == "error":
                args = " ".join(describe_arg(arg) for arg in params.get("args", []))
                errors.append(f"console.error: {args}")
        self.events.clear()
        return errors


def describe_arg(arg: dict[str, Any]) -> str:
    """One console argument as text: its value when it has one, else its description."""
    if "value" in arg:
        value = arg["value"]
        return value if isinstance(value, str) else json.dumps(value)
    return str(arg.get("description", arg.get("unserializableValue", arg.get("type", ""))))


class CleanUrlHandler(http.server.SimpleHTTPRequestHandler):
    """Serve `site/` like Vercel with `cleanUrls`: `/docs/cli` is `docs/cli.html`."""

    extensions_map = {  # noqa: RUF012 (the base class declares it per instance)
        **http.server.SimpleHTTPRequestHandler.extensions_map,
        ".woff2": "font/woff2",
        ".md": "text/markdown; charset=utf-8",
        ".json": "application/json",
        ".svg": "image/svg+xml",
    }

    def translate_path(self, path: str) -> str:
        base = Path(super().translate_path(path))
        html = base.with_name(base.name + ".html")
        if not base.exists() and html.is_file():
            return str(html)
        return str(base)

    def send_error(self, code: int, message: str | None = None, explain: str | None = None) -> None:
        page = Path(self.directory) / "404.html"
        if code == 404 and page.is_file():
            body = page.read_bytes()
            self.send_response(404)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            self.wfile.write(body)
            return
        super().send_error(code, message, explain)

    def log_message(self, format: str, *args: Any) -> None:  # noqa: A002 (stdlib signature)
        return


@contextmanager
def serve_site(root: Path, port: int = 0) -> Iterator[str]:
    handler = functools.partial(CleanUrlHandler, directory=str(root))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", port), handler)
    thread = threading.Thread(target=server.serve_forever, daemon=True)
    thread.start()
    try:
        yield f"http://127.0.0.1:{server.server_address[1]}"
    finally:
        server.shutdown()
        server.server_close()


def fetch_devtools_port(profile: Path, timeout: float) -> Result[int, str]:
    marker = profile / "DevToolsActivePort"
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if marker.is_file():
            first = marker.read_text().splitlines()[:1]
            if first and first[0].isdigit():
                return Ok(int(first[0]))
        time.sleep(0.1)
    return Err(f"Chrome did not open a DevTools port within {timeout:.0f}s")


def stop_chrome(proc: subprocess.Popen[bytes]) -> None:
    proc.terminate()
    try:
        proc.wait(timeout=10)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait()


def remove_profile(profile: Path) -> None:
    """Delete the throwaway profile; a Chrome shutting down may still write to it."""
    for _ in range(10):
        shutil.rmtree(profile, ignore_errors=True)
        if not profile.exists():
            return
        time.sleep(0.5)


@contextmanager
def launch_chrome() -> Iterator[int]:
    """Start headless Chrome on a throwaway profile and yield its DevTools port.

    The profile is deleted however the run ends: Chrome failing to start or
    to open its port, an error, Ctrl-C, or SIGTERM in the caller, or a normal
    finish.
    """
    proc: subprocess.Popen[bytes] | None = None
    profile = Path(tempfile.mkdtemp(prefix="fidryn-shoot-"))
    try:
        proc = subprocess.Popen(
            [
                CHROME,
                *CHROME_FLAGS,
                "--remote-debugging-port=0",
                f"--user-data-dir={profile}",
                "about:blank",
            ],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        match fetch_devtools_port(profile, timeout=30.0):
            case Ok(value=port):
                yield port
            case Err(error=message):
                raise RuntimeError(message)
    finally:
        try:
            if proc is not None:
                stop_chrome(proc)
        finally:
            remove_profile(profile)


def fetch_page_ws(port: int, timeout: float) -> Result[str, str]:
    """The debugger URL of Chrome's first page; retries while Chrome starts up."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/json/list", timeout=5) as resp:
                targets: list[dict[str, Any]] = json.load(resp)
        except OSError:
            time.sleep(0.5)
            continue
        for target in targets:
            if target.get("type") == "page" and "webSocketDebuggerUrl" in target:
                return Ok(str(target["webSocketDebuggerUrl"]))
        time.sleep(0.5)
    return Err(f"Chrome listed no page target within {timeout:.0f}s")


@contextmanager
def open_session(port: int) -> Iterator[Cdp]:
    match fetch_page_ws(port, timeout=90.0):
        case Ok(value=url):
            ws = WebSocket.connect(url)
        case Err(error=message):
            raise RuntimeError(message)
    try:
        cdp = Cdp(ws)
        cdp.call("Page.enable")
        cdp.call("Runtime.enable")
        # Headless Chrome has no focused window, so element.focus() would move
        # activeElement without firing focus events; behave like a focused tab.
        cdp.call("Emulation.setFocusEmulationEnabled", enabled=True)
        yield cdp
    finally:
        ws.close()


def wait_for(cdp: Cdp, condition: str, timeout: float = 15.0) -> bool:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        if cdp.run_js(f"Boolean({condition})"):
            return True
        time.sleep(0.1)
    return False


def load(cdp: Cdp, url: str, width: int, theme: str) -> None:
    cdp.call(
        "Emulation.setDeviceMetricsOverride",
        width=width,
        height=900,
        deviceScaleFactor=1,
        mobile=width < 720,
    )
    cdp.call(
        "Emulation.setEmulatedMedia",
        features=[
            {"name": "prefers-color-scheme", "value": theme},
            {"name": "prefers-reduced-motion", "value": "reduce"},
        ],
    )
    cdp.call("Page.navigate", url=url)
    time.sleep(0.2)
    wait_for(cdp, "document.readyState === 'complete'", timeout=30.0)
    cdp.run_js("document.fonts.ready.then(() => true)", await_promise=True)
    time.sleep(0.25)


def save_shot(cdp: Cdp, path: Path, width: int) -> Path:
    metrics = cdp.call("Page.getLayoutMetrics")
    height = min(int(metrics["cssContentSize"]["height"]), 16000)
    shot = cdp.call(
        "Page.captureScreenshot",
        format="png",
        captureBeyondViewport=True,
        clip={"x": 0, "y": 0, "width": width, "height": max(height, 1), "scale": 1},
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(base64.b64decode(shot["data"]))
    return path


OVERFLOW_JS = r"""
(() => {
  const root = document.documentElement;
  const overflow = root.scrollWidth - root.clientWidth;
  const offenders = [];
  if (overflow > 0) {
    for (const el of document.body.querySelectorAll('*')) {
      const r = el.getBoundingClientRect();
      if (r.width === 0 || r.right <= root.clientWidth + 1) continue;
      let clipped = false;
      for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
        const ox = getComputedStyle(a).overflowX;
        if (ox !== 'visible') { clipped = true; break; }
      }
      if (clipped) continue;
      const cls = typeof el.className === 'string' && el.className.trim()
        ? '.' + el.className.trim().split(/\s+/).join('.') : '';
      offenders.push(el.tagName.toLowerCase() + (el.id ? '#' + el.id : '') + cls);
      if (offenders.length >= 5) break;
    }
  }
  return JSON.stringify({overflow, offenders});
})()
"""


def calc_overflow(cdp: Cdp) -> Result[None, str]:
    data = json.loads(str(cdp.run_js(OVERFLOW_JS)))
    if int(data["overflow"]) > 0:
        return Err(f"scrolls sideways by {data['overflow']}px ({', '.join(data['offenders'])})")
    return Ok(None)


def slug_for(page: str) -> str:
    name = page.strip("/").replace("/", "-")
    return name or "home"


def check_page(cdp: Cdp, label: str, report: Report) -> None:
    match calc_overflow(cdp):
        case Err(error=message):
            report.failures.append(f"{label}: {message}")
        case Ok():
            pass
    for error in cdp.take_errors():
        report.failures.append(f"{label}: JavaScript error: {error}")


# The focused element as one stop of a Tab walk (tag, text, and page position,
# empty when focus is on the page itself; `textarea` when it keeps Tab for
# itself) and what is wrong with its focus ring: not shown, not a 2px outline
# in --rubric, cut off by an ancestor whose overflow is not visible (more than
# a pixel: scroll offsets are whole pixels), or out of view or covered by
# something else. A control that draws no ring of its own (the mill's editor)
# is judged by the nearest ancestor that does.
FOCUS_JS = r"""
(() => {
  const focused = document.activeElement;
  if (!focused || focused === document.body || focused === document.documentElement) {
    return JSON.stringify({stop: '', textarea: false, problems: []});
  }
  const name = (e) => e.tagName.toLowerCase() + (e.id ? '#' + e.id : '') +
    (typeof e.className === 'string' && e.className.trim()
      ? '.' + e.className.trim().split(/\s+/).join('.') : '');
  const swatch = document.createElement('i');
  swatch.style.color = 'var(--rubric)';
  document.body.appendChild(swatch);
  const rubric = getComputedStyle(swatch).color;
  swatch.remove();
  let el = focused;
  if (getComputedStyle(el).outlineStyle === 'none') {
    for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
      if (getComputedStyle(a).outlineStyle !== 'none') { el = a; break; }
    }
  }
  const cs = getComputedStyle(el);
  const width = cs.outlineStyle === 'none' ? 0 : parseFloat(cs.outlineWidth) || 0;
  const grow = Math.max(0, width + (parseFloat(cs.outlineOffset) || 0));
  const boxes = [...el.getClientRects()].filter(r => r.width > 0 || r.height > 0);
  const problems = [];
  if (cs.visibility === 'hidden' || boxes.length === 0) {
    problems.push('focus is on something that is not shown');
  }
  if (width < 2 || cs.outlineColor !== rubric) {
    problems.push('no 2px rubric focus ring (' + cs.outline + ')');
  }
  // In view and on top: the middle of some visible line box of the control
  // (a wrapped link has one per line) must hit the control itself.
  let inView = false, onTop = false, cover = null;
  for (const q of focused.getClientRects()) {
    const x0 = Math.max(0, q.left), x1 = Math.min(innerWidth, q.right);
    const y0 = Math.max(0, q.top), y1 = Math.min(innerHeight, q.bottom);
    if (x1 <= x0 || y1 <= y0) continue;
    inView = true;
    const hit = document.elementFromPoint((x0 + x1) / 2, (y0 + y1) / 2);
    if (!hit || hit === focused || focused.contains(hit)) { onTop = true; break; }
    cover = cover || hit;
  }
  if (!inView) problems.push('focus is out of view');
  else if (!onTop) problems.push('focus is covered by ' + name(cover));
  const f = focused.getBoundingClientRect();
  const top = document.documentElement;
  for (let a = el.parentElement; a && a !== top; a = a.parentElement) {
    const s = getComputedStyle(a);
    if (s.overflowX === 'visible' && s.overflowY === 'visible') continue;
    const r = a.getBoundingClientRect();
    const clip = {
      left: r.left + parseFloat(s.borderLeftWidth),
      right: r.right - parseFloat(s.borderRightWidth),
      top: r.top + parseFloat(s.borderTopWidth),
      bottom: r.bottom - parseFloat(s.borderBottomWidth),
    };
    const cut = new Set();
    for (const q of boxes) {
      const over = {
        left: clip.left - (q.left - grow), right: q.right + grow - clip.right,
        top: clip.top - (q.top - grow), bottom: q.bottom + grow - clip.bottom,
      };
      for (const side of ['left', 'right', 'top', 'bottom']) {
        const axis = side === 'left' || side === 'right' ? s.overflowX : s.overflowY;
        if (axis !== 'visible' && over[side] > 1) {
          cut.add(side + ' ' + over[side].toFixed(1) + 'px');
        }
      }
    }
    if (cut.size) {
      const sides = [...cut].join(', ');
      problems.push('focus ring cut off by ' + name(a) + ' (' + sides + ')');
    }
  }
  const text = (focused.getAttribute('aria-label') || focused.textContent || '')
    .trim().replace(/\s+/g, ' ');
  return JSON.stringify({
    stop: name(focused) + ' ' + JSON.stringify(text.slice(0, 30)) +
      ' at ' + Math.round(f.left + scrollX) + ',' + Math.round(f.top + scrollY),
    textarea: focused.tagName === 'TEXTAREA',
    problems,
  });
})()
"""


def walk_focus(
    cdp: Cdp, label: str, report: Report, *, start_here: bool = False, limit: int = 500
) -> int:
    """Press Tab through the loaded page; every stop must show its whole ring.

    With `start_here`, the element focused now is the first stop, checked
    before the first Tab. In a textarea, Escape comes first: the mill's editor
    keeps Tab for indenting and gives it up after Escape. Stops when focus
    leaves the page or comes back to a stop already seen. Returns the number
    of stops.
    """
    seen: set[str] = set()
    in_textarea = False
    for step in range(limit):
        if step > 0 or not start_here:
            if in_textarea:
                cdp.key("Escape", "Escape", 27)
            cdp.key("Tab", "Tab", 9)
        data = json.loads(str(cdp.run_js(FOCUS_JS)))
        stop = str(data["stop"])
        if not stop or stop in seen:
            break
        seen.add(stop)
        in_textarea = bool(data["textarea"])
        for problem in data["problems"]:
            report.failures.append(f"focus {label}: {stop}: {problem}")
    return len(seen)


def run_site_checks(cdp: Cdp, base: str, report: Report, out: Path, widths: list[int]) -> None:
    """Keyboard search, drawer focus handling, theme toggle, specimen tabs, and
    a Tab walk through every page at each width that checks the focus rings."""

    def expect(ok: bool, what: str) -> None:
        if not ok:
            report.failures.append(f"interaction: {what}")

    load(cdp, f"{base}/docs/outcomes", 1440, "light")
    cdp.key("/", "Slash", 191, "/")
    expect(
        bool(cdp.run_js("document.activeElement && document.activeElement.id === 'site-search'")),
        "'/' focuses the search field",
    )
    cdp.call("Input.insertText", text="outc")
    expect(
        wait_for(cdp, "document.querySelectorAll('#search-results [role=option]').length > 0"),
        "typing 'outc' lists results",
    )
    cdp.key("ArrowDown", "ArrowDown", 40)
    expect(
        bool(
            cdp.run_js(
                "document.querySelectorAll('#search-results [aria-selected=true]').length === 1"
            )
        ),
        "ArrowDown keeps exactly one selected result",
    )
    report.shots.append(save_shot(cdp, out / "interaction-search.png", 1440))
    cdp.key("Escape", "Escape", 27)
    expect(
        bool(cdp.run_js("document.getElementById('search-results').hidden")),
        "Escape closes the results",
    )

    load(cdp, f"{base}/docs/outcomes", 390, "light")
    cdp.run_js("document.querySelector('[data-drawer-open]').click()")
    expect(
        wait_for(cdp, "document.body.classList.contains('drawer-open')"),
        "Menu opens the drawer",
    )
    expect(
        bool(cdp.run_js("document.getElementById('drawer').contains(document.activeElement)")),
        "opening the drawer moves focus into it",
    )
    report.shots.append(save_shot(cdp, out / "interaction-drawer.png", 390))
    cdp.key("Escape", "Escape", 27)
    expect(
        bool(
            cdp.run_js(
                "!document.body.classList.contains('drawer-open') && "
                "document.activeElement === "
                "document.querySelector('[data-drawer-open]')"
            )
        ),
        "Escape closes the drawer and returns focus to Menu",
    )

    load(cdp, f"{base}/", 1440, "light")
    cdp.run_js("document.querySelector('[data-theme-toggle]').click()")
    expect(
        bool(cdp.run_js("document.documentElement.dataset.theme === 'dark'")),
        "the theme toggle switches to dark",
    )
    cdp.run_js(
        "localStorage.removeItem('fidryn-theme'); delete document.documentElement.dataset.theme"
    )
    cdp.run_js("document.querySelector('[role=tab]').focus()")
    cdp.key("ArrowRight", "ArrowRight", 39)
    expect(
        bool(
            cdp.run_js(
                "(() => { const tabs = [...document.querySelectorAll('[role=tab]')];"
                " const panels = tabs.map(t =>"
                " document.getElementById(t.getAttribute('aria-controls')));"
                " return tabs[1].getAttribute('aria-selected') === 'true'"
                " && !panels[1].hidden"
                " && panels.filter(p => !p.hidden).length === 1; })()"
            )
        ),
        "ArrowRight on the specimen tabs shows exactly the second run",
    )
    for error in cdp.take_errors():
        report.failures.append(f"interaction: JavaScript error: {error}")

    for width in widths:
        for page in SITE_PAGES:
            load(cdp, base + page, width, "light")
            label = f"{width} {page}"
            expect(walk_focus(cdp, label, report) > 0, f"Tab reaches nothing on {label}")
            for error in cdp.take_errors():
                report.failures.append(f"focus {label}: JavaScript error: {error}")


def run_site(out: Path, widths: list[int], themes: list[str], with_checks: bool) -> Report:
    report = Report()
    with (
        serve_site(REPO / "site") as base,
        launch_chrome() as port,
        open_session(port) as cdp,
    ):
        for theme in themes:
            for width in widths:
                for page in SITE_PAGES:
                    load(cdp, base + page, width, theme)
                    label = f"{theme} {width} {page}"
                    report.shots.append(
                        save_shot(cdp, out / f"{theme}-{width}-{slug_for(page)}.png", width)
                    )
                    check_page(cdp, label, report)
        if with_checks:
            run_site_checks(cdp, base, report, out, widths)
    return report


MILL_STATES: list[tuple[str, str, str]] = [
    (
        "first",
        "",
        "document.querySelectorAll('#samples [data-sample]').length >= 5"
        " && document.getElementById('editor').value.includes('RequireGate')",
    ),
    (
        "run",
        "document.getElementById('run').click()",
        "document.querySelector('#result-body')"
        " && document.querySelector('#result-body').children.length > 0"
        " && !document.getElementById('run').hasAttribute('aria-busy')",
    ),
    ("table", "document.querySelector('#views [data-view=table]').click()", "true"),
    ("json", "document.querySelector('#views [data-view=json]').click()", "true"),
    (
        "contingent",
        "(() => { document.querySelector('#views [data-view=opinion]').click();"
        " document.querySelector('#samples [data-sample=\"trust-open\"]').click();"
        " })()",
        "document.getElementById('result-body').textContent.includes('Alice')",
    ),
    (
        "diagnostics",
        "(() => { const ed = document.getElementById('editor');"
        " document.querySelector('[data-buffer=module]').click();"
        f" ed.value = {json.dumps(BAD_MODULE)};"
        " ed.dispatchEvent(new Event('input', {bubbles: true}));"
        " setTimeout(() => { const at = ed.value.indexOf('colour') + 2; ed.focus();"
        " ed.setSelectionRange(at, at);"
        " ed.dispatchEvent(new Event('keyup', {bubbles: true}));"
        " ed.dispatchEvent(new Event('click', {bubbles: true}));"
        " document.dispatchEvent(new Event('selectionchange')); }, 1500); })()",
        "document.getElementById('diag-pop') && !document.getElementById('diag-pop').hidden",
    ),
]


def run_mill(url: str, out: Path, widths: list[int], themes: list[str]) -> Report:
    report = Report()
    with launch_chrome() as port, open_session(port) as cdp:
        for theme in themes:
            for width in widths:
                load(cdp, url, width, theme)
                cdp.run_js("localStorage.removeItem('fidryn-mill')")
                load(cdp, url, width, theme)
                for name, action, ready in MILL_STATES:
                    if action:
                        cdp.run_js(action)
                    if not wait_for(cdp, ready, timeout=20.0):
                        report.failures.append(f"{theme} {width} mill {name}: never became ready")
                    time.sleep(0.3)
                    report.shots.append(
                        save_shot(cdp, out / f"{theme}-{width}-mill-{name}.png", width)
                    )
                    check_page(cdp, f"{theme} {width} mill {name}", report)
                # Last, with a result and history on the page: the skip link is
                # focused as a keyboard user would see it and checked as the
                # first stop, then Tab goes round the whole page back to it.
                # Without a skip link the walk would start wherever focus is.
                label = f"{theme} {width} mill"
                if not cdp.run_js("document.querySelector('.skip') !== null"):
                    report.failures.append(f"focus {label}: no skip link")
                cdp.run_js("document.querySelector('.skip')?.focus({focusVisible: true})")
                if walk_focus(cdp, label, report, start_here=True) == 0:
                    report.failures.append(f"focus {label}: Tab reaches nothing")
                for error in cdp.take_errors():
                    report.failures.append(f"focus {label}: JavaScript error: {error}")
                cdp.run_js("localStorage.removeItem('fidryn-mill')")
    return report


def parse_list(text: str) -> list[str]:
    return [part.strip() for part in text.split(",") if part.strip()]


def exit_on_sigterm(signum: int, frame: FrameType | None) -> NoReturn:
    """Turn SIGTERM into SystemExit so the `finally` blocks run.

    By default SIGTERM ends the process on the spot, and Chrome and its profile
    are left behind. 143 is 128 plus SIGTERM, the status a shell reports for it.
    """
    raise SystemExit(143)


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    sub = parser.add_subparsers(dest="command", required=True)
    serve = sub.add_parser("serve", help="serve site/ with clean URLs")
    serve.add_argument("--port", type=int, default=8752)
    for name in ("site", "mill"):
        p = sub.add_parser(name, help=f"screenshot the {name}")
        p.add_argument("--out", type=Path, required=True)
        p.add_argument("--widths", default="390,820,1440")
        p.add_argument("--themes", default="light,dark")
    sub.choices["site"].add_argument(
        "--no-checks", action="store_true", help="skip the interaction checks"
    )
    sub.choices["mill"].add_argument("--url", default="http://127.0.0.1:8751")
    args = parser.parse_args(argv)
    signal.signal(signal.SIGTERM, exit_on_sigterm)

    if args.command == "serve":
        with serve_site(REPO / "site", args.port) as base:
            print(f"serving site/ at {base} (Ctrl-C stops)")
            try:
                while True:
                    time.sleep(3600)
            except KeyboardInterrupt:
                return 0

    widths = [int(w) for w in parse_list(args.widths)]
    themes = parse_list(args.themes)
    run: Callable[[], Report]
    if args.command == "site":
        run = functools.partial(run_site, args.out, widths, themes, not args.no_checks)
    else:
        run = functools.partial(run_mill, args.url, args.out, widths, themes)
    report = run()
    print(f"{len(report.shots)} screenshots in {args.out}")
    for failure in report.failures:
        print(f"FAIL {failure}")
    print("ok" if not report.failures else f"{len(report.failures)} failures")
    return 0 if not report.failures else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
