// Push helper for the SAYL dev workflow (local sandbox only).
//
// This machine's git cannot use the Windows credential store from this session
// (Schannel reports SEC_E_NO_CREDENTIALS), so pushes authenticate with the
// token that `gh` already holds and use the OpenSSL TLS backend. The token is
// passed through git configuration environment variables and is never written
// to disk or printed.
//
// usage: node tools/dev-push.mjs [--remote origin] [--ref HEAD:refs/heads/dev] [--dry-run]
//
// Only the ref named on the command line is pushed; tags and master are not
// pushed by this script.

import { execFileSync, spawnSync } from "node:child_process";

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--dry-run") out.dryRun = true;
    else if (a.startsWith("--")) out[a.slice(2)] = argv[i + 1] && !argv[i + 1].startsWith("--") ? argv[++i] : true;
  }
  return out;
}

const opts = parseArgs(process.argv.slice(2));
const remote = String(opts.remote || "origin");
const ref = String(opts.ref || "HEAD:refs/heads/dev");

let token;
try {
  token = execFileSync("gh", ["auth", "token"], { encoding: "utf8" }).trim();
} catch (err) {
  console.error("gh auth token failed; run `gh auth login` first:", err.message);
  process.exit(1);
}
if (!token) {
  console.error("gh returned an empty token");
  process.exit(1);
}

const basic = Buffer.from(`x-access-token:${token}`, "ascii").toString("base64");
const env = {
  ...process.env,
  GIT_TERMINAL_PROMPT: "0",
  GH_PROMPT_DISABLED: "1",
  GIT_CONFIG_COUNT: "3",
  GIT_CONFIG_KEY_0: "http.sslBackend",
  GIT_CONFIG_VALUE_0: "openssl",
  GIT_CONFIG_KEY_1: "http.https://github.com/.extraheader",
  GIT_CONFIG_VALUE_1: `Authorization: Basic ${basic}`,
  GIT_CONFIG_KEY_2: "credential.helper",
  GIT_CONFIG_VALUE_2: "",
};

const args = ["push"];
if (opts.dryRun) args.push("--dry-run");
args.push("-u", remote, ref);

console.log(`git ${args.join(" ")}`);
const res = spawnSync("git", args, { env, stdio: "inherit" });
process.exit(res.status === null ? 1 : res.status);
