## Summary

Brief description of the changes in this PR.

## Type

- [ ] feat — New feature
- [ ] fix — Bug fix
- [ ] refactor — Code restructuring (no behavior change)
- [ ] perf — Performance improvement
- [ ] test — Adding/modifying tests
- [ ] docs — Documentation only
- [ ] chore — Maintenance, tooling, dependencies
- [ ] build — Build system changes
- [ ] ci — CI configuration changes

## Related Issues

Closes #<issue-number>
Relates to #<issue-number>

## Changes

- Change 1
- Change 2
- ...

## Testing

- [ ] All existing tests pass (`cargo +stable-x86_64-pc-windows-gnu test --workspace`)
- [ ] New unit tests added for new functionality
- [ ] Integration tests pass against `sayl-test-http` fixture server
- [ ] Manual verification completed (describe steps):

```
# Steps to manually verify
1. ...
2. ...
```

## Checklist

### Code Quality
- [ ] `cargo +stable-x86_64-pc-windows-gnu fmt --all -- --check` passes
- [ ] `cargo +stable-x86_64-pc-windows-gnu clippy --workspace --all-targets -- -D warnings` passes
- [ ] No `unsafe` blocks without `// SAFETY:` justification
- [ ] No `unwrap()`/`expect()` in production paths
- [ ] Public APIs documented with `///` comments

### Architecture
- [ ] Changes respect crate boundaries (core: no I/O, storage: only file I/O, http: only network I/O)
- [ ] No new external dependencies without justification (prefer pure Rust, avoid C deps)
- [ ] No network calls in tests — all HTTP tests use `sayl-test-http`

### Security & Privacy
- [ ] No secrets, tokens, or credentials in code or commit history
- [ ] No telemetry, analytics, or tracking code added
- [ ] Input validation at boundaries (filenames, URLs, headers, plugin manifests)
- [ ] Cross-origin redirect handling strips Authorization/Cookie headers
- [ ] Plugin/WASM capabilities follow least-privilege principle

### Documentation
- [ ] README.md updated if user-facing change
- [ ] Crate-level README.md updated if crate API changed
- [ ] Architecture documentation updated if architectural change
- [ ] CHANGELOG.md updated (when it exists)

### Git Hygiene
- [ ] Commit messages follow English imperative style (`feat(scope): subject`)
- [ ] No destructive git operations (`reset --hard`, `checkout --`, `push --force`)
- [ ] Commits are logically grouped and atomic

### Accessibility (UI changes)
- [ ] WCAG AA contrast maintained
- [ ] RTL/LTR support verified
- [ ] Focus rings visible (`:focus-visible`)
- [ ] `prefers-reduced-motion` respected
- [ ] ARIA labels/roles correct
- [ ] Keyboard navigation works

## Screenshots / Recordings (UI changes)

| Before | After |
|--------|-------|
| ![before](url) | ![after](url) |

## Additional Notes

Any additional context, concerns, or discussion points for reviewers.