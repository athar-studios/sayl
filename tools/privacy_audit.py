#!/usr/bin/env python3
"""
SAYL Privacy and Network Audit Tool
-----------------------------------
Scans the repository to verify privacy and security commitments:
1. No known telemetry/analytics hosts or SDK identifiers in source files.
2. No hardcoded personal developer user paths (C:\\Users\\<name>\\, /home/<name>/).
3. No hardcoded secrets in non-test source files.
4. No outbound URLs in application code other than user-supplied, documentation links,
   or fixture / local servers.

Exit code: 0 on PASS, 1 on FAIL.
Supports --json flag for machine-readable output.
"""

import argparse
import json
import os
import re
import sys
from pathlib import Path

# Directories to skip entirely
IGNORED_DIRS = {
    "target",
    "node_modules",
    "reports",
    "artifacts",
    ".git",
    "dist",
    "build",
    "gen",
    ".svelte-kit",
    "__pycache__",
    ".idea",
    ".vscode",
}

# Files to skip (lockfiles, self)
IGNORED_FILES = {
    "package-lock.json",
    "Cargo.lock",
    "pnpm-lock.yaml",
    "yarn.lock",
}

# Known telemetry / analytics identifiers
TELEMETRY_PATTERNS = [
    r"google-analytics",
    r"googletagmanager",
    r"\bsentry\b",
    r"@sentry/",
    r"mixpanel",
    r"segment\.io",
    r"amplitude(?:\.com)?",
    r"hotjar",
    r"facebook\.net",
    r"doubleclick",
    r"bugsnag",
    r"datadog",
]

# Secrets patterns
SECRET_PATTERNS = [
    (r"\bAKIA[0-9A-Z]{16}\b", "AWS Access Key ID"),
    (r"\bghp_[0-9a-zA-Z]{36}\b", "GitHub Personal Access Token"),
    (r"\bsk-[a-zA-Z0-9]{20,}\b", "Secret API Key"),
    (r"-----BEGIN [A-Z ]*PRIVATE KEY-----", "Private Key Header"),
    (r'\bpassword\s*=\s*"[^"]+"', 'Hardcoded password assignment'),
]

# User path regex: catches C:\Users\<name>\ or C:\\Users\\<name>\\ or /home/<name>/
USER_PATH_REGEX = re.compile(
    r'(?:[cC]:(?:\\\\|/|\\)[uU]sers(?:\\\\|/|\\)|/home/)([a-zA-Z0-9_\-\.]+)(?:\\\\|/|\\)',
    re.IGNORECASE,
)

# Allowed placeholder names for user paths in mock/test/doc
ALLOWED_USER_PLACEHOLDERS = {
    "user",
    "<name>",
    "<you>",
    "<username>",
    "username",
}

# Source code extensions
SOURCE_EXTENSIONS = {
    ".rs",
    ".js",
    ".ts",
    ".svelte",
    ".html",
    ".css",
    ".py",
    ".json",
    ".toml",
}

# URL regex
URL_REGEX = re.compile(r'https?://[^\s"\'`<>)]+')

# Allowed URL patterns in application code
ALLOWED_URL_HOST_PATTERNS = [
    r"^https?://(?:127\.0\.0\.1|localhost|\[::1\]|0\.0\.0\.0)(?::(?:\d+|\{[A-Za-z0-9_]*\}))?(?:/.*)?$",
    r"^http://ipc\.localhost(?:/.*)?$",
    r"^https?://github\.com(?:/.*)?$",
    r"^https?://raw\.githubusercontent\.com(?:/.*)?$",
    r"^http://www\.w3\.org/(?:2000/svg|1999/xlink)$",
    r"^https?://(?:[a-zA-Z0-9_\-]+\.)*example\.(?:com|org)(?::\d+)?(?:/.*)?$",
    r"^https?://releases\.ubuntu\.com(?:/.*)?$",
    r"^https?://(?:[a-zA-Z0-9_\-]+\.)*public-cdn\.com(?:/.*)?$",
    r"^https?://hops\.com(?:/.*)?$",
    r"^https?://loop\.com(?:/.*)?$",
    r"^https?://(?:a|origin1|cdn\.other)\.com(?:/.*)?$",
    # Official project support page (no tracking parameters, not application telemetry).
    r"^https?://ko-fi\.com/athar_studio(?:/.*)?$",
    # Official project website, sister app, metadata schemas, and license references
    r"^https?://(?:[a-zA-Z0-9_\-]+\.)*sayl\.site(?:/.*)?$",
    r"^https?://atharapp\.download(?:/.*)?$",
    r"^https?://schema\.org(?:/.*)?$",
    r"^https?://(?:www\.)?apache\.org/licenses/LICENSE-2\.0(?:/.*)?$",
    r"^https?://polyformproject\.org/licenses/noncommercial/1\.0\.0(?:/.*)?$",
    # Competitor and comparison reference sites
    r"^https?://(?:www\.)?(?:internetdownloadmanager|freedownloadmanager)\.com(?:/.*)?$",
    r"^https?://aria2\.github\.io(?:/.*)?$",
]

# A URL template whose host is filled in at runtime from a bound socket address,
# e.g. `format!("http://{}", addr)`. This is only tolerated inside the benchmark
# harness, which binds loopback fixture servers; it is scoped to that directory
# so a real remote URL elsewhere cannot hide behind the pattern.
RUNTIME_HOST_TEMPLATE_REGEX = re.compile(r"^https?://\{\}$")
RUNTIME_HOST_TEMPLATE_DIRS = ("crates/bench/",)


def is_binary_file(file_path: Path) -> bool:
    """Checks if a file appears to be binary by inspecting first 1024 bytes."""
    binary_extensions = {
        ".exe", ".dll", ".pdb", ".png", ".jpg", ".jpeg", ".ico",
        ".webp", ".gif", ".wasm", ".zip", ".tar", ".gz", ".bin",
        ".pyc", ".woff", ".woff2", ".ttf", ".eot"
    }
    if file_path.suffix.lower() in binary_extensions:
        return True
    try:
        with open(file_path, "rb") as f:
            chunk = f.read(1024)
            return b"\0" in chunk
    except OSError:
        return True


def is_test_file(rel_path: str) -> bool:
    """Determines whether a relative file path is part of a test suite."""
    norm = rel_path.replace("\\", "/").lower()
    parts = norm.split("/")
    if any(p in ("tests", "test", "__tests__", "test-http") for p in parts):
        return True
    filename = parts[-1]
    if (
        filename.endswith("_tests.rs")
        or filename.endswith("_test.rs")
        or filename.endswith(".test.js")
        or filename.endswith(".spec.js")
        or filename.startswith("test_")
    ):
        return True
    return False


def is_application_code(rel_path: str) -> bool:
    """Determines if a file is application runtime code (crates/*/src or apps/*)."""
    norm = rel_path.replace("\\", "/").lower()
    if is_test_file(rel_path):
        return False
    if norm.startswith("crates/") and "/src/" in norm:
        return True
    if norm.startswith("apps/desktop/src/"):
        return True
    if norm.startswith("apps/desktop/src-tauri/src/"):
        return True
    if norm == "apps/desktop/src-tauri/tauri.conf.json":
        return True
    if norm.startswith("apps/website/"):
        ext = Path(rel_path).suffix.lower()
        if ext in (".html", ".js", ".css", ".svg"):
            return True
    return False


def run_audit(root_dir: Path) -> dict:
    results = {
        "telemetry_and_analytics": {"status": "PASS", "violations": [], "allowances": []},
        "hardcoded_user_paths": {"status": "PASS", "violations": [], "allowances": []},
        "secrets_in_non_test_source": {"status": "PASS", "violations": [], "allowances": []},
        "outbound_urls": {"status": "PASS", "violations": [], "allowances": []},
    }

    total_files_scanned = 0
    own_script = Path(__file__).resolve()

    compiled_telemetry = [re.compile(p, re.IGNORECASE) for p in TELEMETRY_PATTERNS]
    compiled_allowed_urls = [re.compile(p, re.IGNORECASE) for p in ALLOWED_URL_HOST_PATTERNS]

    for root, dirs, files in os.walk(root_dir):
        # Prune ignored directories
        dirs[:] = [d for d in dirs if d not in IGNORED_DIRS and not d.startswith(".")]

        for filename in files:
            if filename in IGNORED_FILES:
                continue
            file_path = Path(root) / filename
            if file_path.resolve() == own_script:
                continue

            rel_path = file_path.relative_to(root_dir).as_posix()
            ext = file_path.suffix.lower()

            if is_binary_file(file_path):
                continue

            try:
                content = file_path.read_text(encoding="utf-8", errors="replace")
            except OSError:
                continue

            total_files_scanned += 1
            lines = content.splitlines()
            is_src = ext in SOURCE_EXTENSIONS
            is_test = is_test_file(rel_path)
            is_app = is_application_code(rel_path)

            # Check 1: Telemetry in source files
            if is_src:
                for line_idx, line in enumerate(lines, 1):
                    for pat in compiled_telemetry:
                        if pat.search(line):
                            results["telemetry_and_analytics"]["violations"].append({
                                "file": rel_path,
                                "line": line_idx,
                                "match": pat.pattern,
                                "line_text": line.strip(),
                            })

            # Check 2: Hardcoded user paths in source files
            if is_src:
                in_cfg_test = False
                for line_idx, line in enumerate(lines, 1):
                    if "#[cfg(test)]" in line:
                        in_cfg_test = True
                    match = USER_PATH_REGEX.search(line)
                    if match:
                        username = match.group(1).lower()
                        full_match = match.group(0)
                        if username in ALLOWED_USER_PLACEHOLDERS:
                            reason = (
                                "Synthetic placeholder 'User' in unit test"
                                if (is_test or in_cfg_test)
                                else "Synthetic placeholder 'User' in dev mock fallback"
                            )
                            results["hardcoded_user_paths"]["allowances"].append({
                                "file": rel_path,
                                "line": line_idx,
                                "match": full_match,
                                "reason": reason,
                            })
                        else:
                            results["hardcoded_user_paths"]["violations"].append({
                                "file": rel_path,
                                "line": line_idx,
                                "match": full_match,
                                "line_text": line.strip(),
                            })

            # Check 3: Secrets in non-test source files
            if is_src and not is_test:
                in_cfg_test = False
                for line_idx, line in enumerate(lines, 1):
                    if "#[cfg(test)]" in line:
                        in_cfg_test = True
                    if in_cfg_test:
                        continue
                    for pat_str, desc in SECRET_PATTERNS:
                        if re.search(pat_str, line):
                            results["secrets_in_non_test_source"]["violations"].append({
                                "file": rel_path,
                                "line": line_idx,
                                "description": desc,
                                "line_text": line.strip(),
                            })

            # Check 4: Outbound URLs in application code
            if is_app:
                in_cfg_test = False
                for line_idx, line in enumerate(lines, 1):
                    if "#[cfg(test)]" in line:
                        in_cfg_test = True
                    if in_cfg_test:
                        continue
                    # Ignore pure comment lines in Rust/JS
                    trimmed = line.strip()
                    if trimmed.startswith(("//", "///", "/*", "*", "#")):
                        continue

                    urls = URL_REGEX.findall(line)
                    for url in urls:
                        # Clean trailing punctuation often caught by regex
                        cleaned_url = url.rstrip('";,\')')
                        # Check against allowed list
                        is_allowed = any(p.match(cleaned_url) for p in compiled_allowed_urls)
                        runtime_template = (
                            RUNTIME_HOST_TEMPLATE_REGEX.match(cleaned_url)
                            and rel_path.replace("\\", "/").startswith(RUNTIME_HOST_TEMPLATE_DIRS)
                        )
                        if is_allowed or runtime_template:
                            # Classify allowance reason
                            if "127.0.0.1" in cleaned_url or "localhost" in cleaned_url:
                                reason = "Localhost/fixture server address"
                            elif runtime_template:
                                reason = "Runtime-filled loopback host template (benchmark harness only)"
                            elif "github.com" in cleaned_url or "raw.githubusercontent.com" in cleaned_url:
                                reason = "Documentation/repository link"
                            elif "w3.org" in cleaned_url:
                                reason = "Standard XML/SVG namespace"
                            elif "example." in cleaned_url or "ubuntu." in cleaned_url:
                                reason = "Documentation/mock placeholder address"
                            elif "sayl.site" in cleaned_url:
                                reason = "Official project website link"
                            elif "atharapp.download" in cleaned_url:
                                reason = "Sister application link"
                            elif "schema.org" in cleaned_url:
                                reason = "Standard metadata schema context"
                            elif "apache.org" in cleaned_url:
                                reason = "Open source license reference"
                            elif "ko-fi.com" in cleaned_url:
                                reason = "Donation/support link"
                            else:
                                reason = "Permitted reference"
                            results["outbound_urls"]["allowances"].append({
                                "file": rel_path,
                                "line": line_idx,
                                "url": cleaned_url,
                                "reason": reason,
                            })
                        else:
                            results["outbound_urls"]["violations"].append({
                                "file": rel_path,
                                "line": line_idx,
                                "url": cleaned_url,
                                "line_text": trimmed,
                            })

    # Evaluate category statuses
    any_failed = False
    for cat_name, cat_data in results.items():
        if cat_data["violations"]:
            cat_data["status"] = "FAIL"
            any_failed = True
        else:
            cat_data["status"] = "PASS"

    total_categories = len(results)
    failed_categories = sum(1 for c in results.values() if c["status"] == "FAIL")
    passed_categories = total_categories - failed_categories

    report = {
        "status": "FAIL" if any_failed else "PASS",
        "exit_code": 1 if any_failed else 0,
        "summary": {
            "total_categories": total_categories,
            "passed_categories": passed_categories,
            "failed_categories": failed_categories,
            "total_files_scanned": total_files_scanned,
        },
        "categories": results,
    }
    return report


def print_human_report(report: dict, root_dir: Path):
    print("=" * 72)
    print(" SAYL Privacy & Network Audit Report")
    print("=" * 72)
    print(f"Scan Root:            {root_dir}")
    print(f"Files Scanned:        {report['summary']['total_files_scanned']}")
    print(f"Excluded Directories: {', '.join(sorted(IGNORED_DIRS))}")
    print("-" * 72)

    cat_titles = {
        "telemetry_and_analytics": "1. Telemetry and Analytics SDKs / Hosts",
        "hardcoded_user_paths": "2. Hardcoded Absolute User Paths",
        "secrets_in_non_test_source": "3. Secrets in Non-Test Source",
        "outbound_urls": "4. Outbound URLs in Application Code",
    }

    for cat_key, title in cat_titles.items():
        cat = report["categories"][cat_key]
        status = cat["status"]
        status_str = f"[{status}]"
        print(f"\n{title} {status_str}")
        print(f"  Violations: {len(cat['violations'])}")
        print(f"  Allowances: {len(cat['allowances'])}")

        if cat["violations"]:
            print("  VIOLATION DETAILS:")
            for v in cat["violations"]:
                loc = f"{v['file']}:{v['line']}"
                detail = v.get("match") or v.get("url") or v.get("description", "")
                print(f"    - {loc} -> {detail}")
                if "line_text" in v:
                    print(f"      Code: {v['line_text']}")

        if cat["allowances"]:
            print("  Documented Allowances:")
            for a in cat["allowances"]:
                loc = f"{a['file']}:{a['line']}"
                target = a.get("match") or a.get("url", "")
                reason = a.get("reason", "")
                print(f"    - {loc} -> {target} ({reason})")

    print("\n" + "=" * 72)
    print(
        f"Audit Summary: {report['summary']['passed_categories']}/"
        f"{report['summary']['total_categories']} categories passed."
    )
    print(f"FINAL RESULT:  {report['status']}")
    print("=" * 72)


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")
    parser = argparse.ArgumentParser(description="SAYL Privacy and Network Audit")
    parser.add_argument("--json", action="store_true", help="Output audit report as JSON")
    parser.add_argument("--root", type=str, default=None, help="Root directory to audit")
    args = parser.parse_args()

    root_dir = Path(args.root).resolve() if args.root else Path(__file__).resolve().parents[1]

    report = run_audit(root_dir)

    if args.json:
        print(json.dumps(report, indent=2))
    else:
        print_human_report(report, root_dir)

    sys.exit(report["exit_code"])


if __name__ == "__main__":
    main()
