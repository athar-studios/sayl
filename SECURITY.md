# Security Policy

## Supported Versions

SAYL is in pre-release development (pre-M1). **No versions are currently supported for security updates.**

| Version | Supported |
|---------|-----------|
| Pre-M1 (development) | ❌ No |
| M1+ (future) | TBD |

Once a stable release is published, this section will be updated with the supported version range and security update policy.

## Reporting a Vulnerability

**Do not report security vulnerabilities via public GitHub Issues.**

If you discover a security vulnerability, please report it **privately** by emailing:

**security@sayl.example.com** (placeholder — replace with actual contact when established)

Include the following information:
- Description of the vulnerability
- Steps to reproduce (if applicable)
- Potential impact
- Any suggested mitigation or fix
- Your contact information for follow-up

We will acknowledge receipt within 72 hours and provide a preliminary assessment within 7 days. We aim to resolve verified vulnerabilities before the next release.

## Scope

### In Scope

The following are considered in scope for vulnerability reports:

- **Memory safety issues** in Rust code (use-after-free, buffer overflows, etc.) — though Rust's safety guarantees make these rare in safe code
- **HTTP transport vulnerabilities**: redirect handling, header injection, range validation bypasses, certificate validation failures
- **File system vulnerabilities**: path traversal in filename sanitization, symlink races, TOCTOU in file operations
- **Crash-safety violations**: data corruption on power loss, incomplete writes marked as complete, journal/snapshot inconsistency
- **Deserialization issues**: malformed input causing panic or excessive resource consumption
- **Concurrency bugs**: race conditions in scheduler, writer, or storage actors
- **Plugin/WASM host vulnerabilities** (when implemented): sandbox escape, capability bypass, resource exhaustion
- **Browser extension bridge** (when implemented): Native Messaging schema validation, origin verification
- **Update mechanism** (when implemented): signature verification, rollback protection, downgrade attacks

### Out of Scope

The following are explicitly **out of scope**:

- **Third-party penetration testing** of external services (GitHub, file hosts, CDNs)
- **Data exfiltration testing** — we do not authorize attempts to extract data from user machines
- **Social engineering** or phishing attacks
- **Physical access** attacks
- **Side-channel attacks** (timing, power, EM) unless they reveal a concrete code vulnerability
- **Denial of service** via resource exhaustion on the user's own machine (local DoS)
- **Vulnerabilities in dependencies** — report those to the dependency maintainers (we track via `cargo audit` and `dependabot`)
- **Issues requiring user misconfiguration** (e.g., disabling TLS validation manually)
- **Speculative threats** without a reproducible proof-of-concept

## Privacy Promise

**SAYL contains no telemetry, analytics, tracking, or automatic data collection.**

- No usage analytics, no crash reporters, no error reporting services
- No external fonts, scripts, stylesheets, or embedded widgets
- No device fingerprinting, no unique identifiers generated or transmitted
- Download data (URLs, filenames, progress) is processed **locally only**
- External network connections occur **only** for:
  1. The actual file download (user-initiated)
  2. Optional update checks (opt-in, manual trigger)
  3. Optional plugin/theme catalog refresh (opt-in, manual trigger)
  4. Browser extension Native Messaging (user-installed, explicit permission)
- Logs are local, user-deletable, and exportable after redacting sensitive data

See [README.md](README.md) for the full privacy statement.

## Security Review Process

**A security review happens before every release.**

1. **Automated checks** (CI):
   - `cargo audit` — vulnerability scanning for dependencies
   - `cargo deny` — license, bans, sources checking
   - `clippy` with `-D warnings` — catch unsafe patterns
   - `fmt` — consistent formatting (prevents homoglyph attacks)

2. **Manual review** (pre-release):
   - Code review of all changes since last release
   - Focus on: HTTP transport, file I/O, plugin host, extension bridge, update mechanism
   - Threat modeling for new features
   - Dependency review for new crates

3. **Strix/security tooling** (when available):
   - Authorized scans on owned staging assets only
   - No third-party scanners without owner approval
   - No exfiltration, no destructive tests

4. **Release gate**:
   - No critical/high severity findings unresolved
   - All findings documented with confidence, reproducibility, impact, fix, retest
   - Owner approval required

## Secure Development Practices

- **No `unsafe`** unless absolutely necessary and documented with `// SAFETY:` comments
- **No `unwrap()`/`expect()`** in production paths — all `Result` handled
- **Input validation** at boundaries (HTTP headers, filenames, URLs, plugin manifests)
- **Principle of least privilege** — plugins get minimal capabilities, no default FS/network/env
- **DNS rebinding defense** — plugin network allowlist validates resolved IPs
- **Secret handling** — no Authorization/Cookie leakage on cross-origin redirects
- **Deterministic testing** — `sayl-test-http` fixture server, no flaky network tests

## Contact

For security-related questions (not vulnerability reports), open a GitHub Discussion or Issue with the `security` label.

---

*This policy is a living document and will be updated as the project matures. The project owner has final authority on scope and process decisions.*