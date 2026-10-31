// SAYL browser bridge against the real `sayl.exe` CLI.
//
// Windows has no FIFO, so the bridge keeps a small NDJSON queue file and
// spawns one detached CLI process per requested download:
//
//   node tools/e2e/bridge-cli.mjs add <url> [--state DIR] [--sayl EXE] [--download-dir DIR]
//   node tools/e2e/bridge-cli.mjs status [--state DIR]
//   node tools/e2e/bridge-cli.mjs serve [--state DIR] [--sayl EXE]  (prints a loopback URL)
//
// Exit codes: 0 ok, 2 usage error, 3 the CLI refused the download.
// This is a test/e2e helper: it is not part of the shipped installer and must
// never be registered as a browser native-messaging host.

import { spawn } from "node:child_process";
import fs from "node:fs";
import http from "node:http";
import path from "node:path";

const USAGE = `usage:
  node tools/e2e/bridge-cli.mjs add <url> [--state DIR] [--sayl EXE] [--download-dir DIR] [--data-dir DIR]
  node tools/e2e/bridge-cli.mjs status [--state DIR]
  node tools/e2e/bridge-cli.mjs serve [--state DIR] [--sayl EXE] [--port 0]`;

function parseArgs(argv) {
  const out = { _: [] };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a.startsWith("--")) {
      const key = a.slice(2);
      const val = argv[i + 1];
      if (val === undefined || val.startsWith("--")) {
        out[key] = true;
      } else {
        out[key] = val;
        i++;
      }
    } else {
      out._.push(a);
    }
  }
  return out;
}

function queuePath(opts) {
  const stateDir = opts.state || path.join("target", "e2e-state");
  fs.mkdirSync(stateDir, { recursive: true });
  return path.join(stateDir, "browser-queue.ndjson");
}

function saylExe(opts) {
  return path.resolve(opts.sayl || path.join("target", "release", "sayl.exe"));
}

function appendQueue(opts, entry) {
  fs.appendFileSync(queuePath(opts), JSON.stringify(entry) + "\n", "utf8");
}

function readQueue(opts) {
  const p = queuePath(opts);
  if (!fs.existsSync(p)) return [];
  return fs
    .readFileSync(p, "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((line) => {
      try {
        return JSON.parse(line);
      } catch {
        return { raw: line, error: "unparseable queue line" };
      }
    });
}

function isWebUrl(value) {
  try {
    const u = new URL(value);
    return u.protocol === "http:" || u.protocol === "https:";
  } catch {
    return false;
  }
}

async function addOne(opts, url, extra) {
  if (!isWebUrl(url)) {
    return { ok: false, code: 2, message: `refusing non-http(s) URL: ${url}` };
  }
  const exe = saylExe(opts);
  if (!fs.existsSync(exe)) {
    return { ok: false, code: 3, message: `sayl executable not found: ${exe}` };
  }

  const args = ["add", url, "--json"];
  if (opts["download-dir"]) args.push("--download-dir", path.resolve(opts["download-dir"]));
  if (opts["data-dir"]) args.push("--data-dir", path.resolve(opts["data-dir"]));

  const result = await new Promise((resolve) => {
    // stdio: "ignore" keeps the detached child free of inherited handles.
    const child = spawn(exe, args, { detached: true, stdio: "ignore", windowsHide: true });
    let exitCode = null;
    child.on("error", (err) => resolve({ ok: false, code: 3, message: err.message }));
    child.on("exit", (code) => (exitCode = code));
    // Do not block the browser on long downloads: report acceptance once the
    // process is alive, and record the exit code when it settles.
    resolve({ ok: true, code: 0, message: "dispatched", pid: child.pid, exitCode });
    child.unref();
  });

  appendQueue(opts, {
    at: new Date().toISOString(),
    url,
    referrer: extra?.referrer ?? null,
    source: extra?.source ?? "cli",
    result,
  });
  return result;
}

function emit(payload, code = 0) {
  process.stdout.write(JSON.stringify(payload) + "\n");
  process.exitCode = code;
}

async function main() {
  const opts = parseArgs(process.argv.slice(2));
  const cmd = opts._[0];

  if (!cmd || cmd === "help") {
    process.stdout.write(USAGE + "\n");
    emit({ ok: false, code: 2, message: "usage" }, 2);
    return;
  }

  if (cmd === "add") {
    const url = opts._[1];
    if (!url) {
      emit({ ok: false, code: 2, message: "missing url" }, 2);
      return;
    }
    const r = await addOne(opts, url);
    emit({ command: "add", url, ...r }, r.code);
    return;
  }

  if (cmd === "status") {
    emit({ ok: true, command: "status", entries: readQueue(opts) }, 0);
    return;
  }

  if (cmd === "serve") {
    const port = Number(opts.port || 0);
    const server = http.createServer(async (req, res) => {
      const url = new URL(req.url, "http://127.0.0.1");
      res.setHeader("content-type", "application/json; charset=utf-8");
      try {
        if (req.method === "POST" && url.pathname === "/add") {
          const query = url.searchParams.get("url");
          if (!query) {
            res.statusCode = 400;
            res.end(JSON.stringify({ ok: false, message: "missing ?url" }));
            return;
          }
          const r = await addOne(opts, query, { source: "http-bridge" });
          res.statusCode = r.ok ? 200 : 500;
          res.end(JSON.stringify(r));
          return;
        }
        if (url.pathname === "/status") {
          res.end(JSON.stringify({ ok: true, entries: readQueue(opts) }));
          return;
        }
        res.statusCode = 404;
        res.end(JSON.stringify({ ok: false, message: "not found" }));
      } catch (err) {
        res.statusCode = 500;
        res.end(JSON.stringify({ ok: false, message: String(err) }));
      }
    });
    server.listen(port, "127.0.0.1", () => {
      emit({ ok: true, command: "serve", bridge: `http://127.0.0.1:${server.address().port}` }, 0);
    });
    return;
  }

  emit({ ok: false, code: 2, message: `unknown command: ${cmd}` }, 2);
}

main().catch((err) => {
  process.stderr.write(String(err && err.stack ? err.stack : err) + "\n");
  process.exitCode = 3;
});
