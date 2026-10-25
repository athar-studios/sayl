#!/usr/bin/env python3
"""
Captures real application screenshots of SAYL desktop app in Arabic and English
using headless Chrome against the built web distribution.
Saves screenshots into docs/screenshots/v0.1.0/ and docs/screenshots/.
"""

import asyncio
import base64
import functools
import http.server
import json
import os
import shutil
import socketserver
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request
from pathlib import Path

import websockets

REPO_ROOT = Path(__file__).resolve().parents[1]
DIST_DIR = REPO_ROOT / "apps" / "desktop" / "dist"
OUTPUT_DIR_V1 = REPO_ROOT / "docs" / "screenshots" / "v0.1.0"
OUTPUT_DIR_ROOT = REPO_ROOT / "docs" / "screenshots"
CHROME_PATH = r"C:\Program Files\Google\Chrome\Application\chrome.exe"
HTTP_PORT = 5199
CDP_PORT = 9222


def run_http_server(dist_dir, port):
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(dist_dir))
    with socketserver.TCPServer(("", port), handler) as httpd:
        httpd.serve_forever()


_req_id = 0

async def cdp_call(ws, method, params=None):
    global _req_id
    _req_id += 1
    msg_id = _req_id
    payload = {"id": msg_id, "method": method}
    if params:
        payload["params"] = params
    await ws.send(json.dumps(payload))
    while True:
        raw = await ws.recv()
        resp = json.loads(raw)
        if resp.get("id") == msg_id:
            return resp.get("result", {})


async def capture_page(ws, url, out_v1, out_root, is_en=False):
    print(f"Navigating to {url}...", flush=True)
    await cdp_call(ws, "Page.enable")
    await cdp_call(ws, "Runtime.enable")
    await cdp_call(ws, "Log.enable")
    await cdp_call(ws, "Page.navigate", {"url": url})

    # Wait for page to load and render table
    print("Waiting for application to mount and render data grid...", flush=True)
    for i in range(50):
        await asyncio.sleep(0.3)
        res = await cdp_call(
            ws,
            "Runtime.evaluate",
            {
                "expression": """
                (() => {
                    const html = document.body.innerHTML;
                    const splash = document.querySelector('.splash-screen');
                    const table = document.querySelector('.tasks-table');
                    const rows = document.querySelectorAll('.grid-row');
                    return {
                        htmlLength: html ? html.length : 0,
                        hasTable: !!table,
                        rowCount: rows ? rows.length : 0,
                        splash: !!splash
                    };
                })()
                """,
                "returnByValue": True,
            },
        )
        val = res.get("result", {}).get("value", {})
        if i % 5 == 0:
            print(f"Status check {i}: {val}", flush=True)
        if val.get("hasTable") and val.get("rowCount", 0) > 0:
            print(f"Data grid loaded with {val.get('rowCount')} tasks.", flush=True)
            break

    # If English, ensure language switch and LTR dir are active
    if is_en:
        await cdp_call(
            ws,
            "Runtime.evaluate",
            {
                "expression": """
                (() => {
                    document.documentElement.dir = 'ltr';
                    document.documentElement.lang = 'en';
                })()
                """,
            },
        )

    await asyncio.sleep(1.0)

    print("Capturing screenshot via Page.captureScreenshot...", flush=True)
    shot_res = await cdp_call(ws, "Page.captureScreenshot", {"format": "png"})
    data = shot_res.get("data")
    if not data:
        raise RuntimeError("No screenshot data returned from Chrome CDP")

    raw_bytes = base64.b64decode(data)
    out_v1.parent.mkdir(parents=True, exist_ok=True)
    out_root.parent.mkdir(parents=True, exist_ok=True)

    out_v1.write_bytes(raw_bytes)
    out_root.write_bytes(raw_bytes)
    print(f"Saved: {out_v1} ({len(raw_bytes)} bytes)")
    print(f"Saved: {out_root}")


async def run_capture():
    if not DIST_DIR.exists():
        print(f"Error: {DIST_DIR} not found. Run 'npm run build --prefix apps/desktop' first.", file=sys.stderr)
        sys.exit(1)

    print(f"Starting HTTP server on port {HTTP_PORT}...")
    server_thread = threading.Thread(target=run_http_server, args=(DIST_DIR, HTTP_PORT), daemon=True)
    server_thread.start()
    time.sleep(1.0)

    user_data_dir = tempfile.mkdtemp(prefix="sayl_chrome_")

    chrome_cmd = [
        CHROME_PATH,
        "--headless=new",
        f"--remote-debugging-port={CDP_PORT}",
        f"--user-data-dir={user_data_dir}",
        "--disable-extensions",
        "--disable-gpu",
        "--no-sandbox",
        "--hide-scrollbars",
        "--window-size=1280,820",
        "about:blank",
    ]

    print("Starting Chrome headless...")
    proc = subprocess.Popen(chrome_cmd)
    try:
        # Wait for debugging port
        ws_url = None
        for _ in range(30):
            time.sleep(0.5)
            try:
                req = urllib.request.urlopen(f"http://127.0.0.1:{CDP_PORT}/json/list", timeout=2)
                tabs = json.loads(req.read().decode())
                page_tabs = [t for t in tabs if t.get("type") == "page" and "webSocketDebuggerUrl" in t]
                if page_tabs:
                    ws_url = page_tabs[0]["webSocketDebuggerUrl"]
                    break
            except Exception:
                pass

        if not ws_url:
            raise RuntimeError("Failed to obtain Chrome CDP WebSocket URL")

        print(f"Connecting to CDP: {ws_url}")
        async with websockets.connect(ws_url) as ws:
            # 1. Arabic RTL Screenshot
            await capture_page(
                ws,
                f"http://localhost:{HTTP_PORT}/?lang=ar",
                OUTPUT_DIR_V1 / "main-window-ar.png",
                OUTPUT_DIR_ROOT / "main-window-ar.png",
                is_en=False,
            )

            # 2. English LTR Screenshot
            await capture_page(
                ws,
                f"http://localhost:{HTTP_PORT}/?lang=en",
                OUTPUT_DIR_V1 / "main-window-en.png",
                OUTPUT_DIR_ROOT / "main-window-en.png",
                is_en=True,
            )

    finally:
        print("Stopping Chrome...")
        proc.terminate()
        try:
            proc.wait(timeout=5)
        except Exception:
            proc.kill()
        shutil.rmtree(user_data_dir, ignore_errors=True)


if __name__ == "__main__":
    asyncio.run(run_capture())
