# Contributing — CipherLayer Browser

## License and DCO

Contributions are accepted under [Apache-2.0](LICENSE). Every commit must carry a
`Signed-off-by` line (`git commit -s`), certifying the [Developer Certificate of
Origin](https://developercertificate.org). Security findings: see `REVIEW-GUIDE.md`
in the family repo.

## Ground rules

1. Honor the project's binding constraints on every contribution —
   in particular the wallet trust-core boundary (no Vault, no signer, no key
   handling here) and the gate limitations in `docs/architecture/browser-mvp-gate.md`.
2. Dependency direction: scam-guard → wallet → browser. Consume sibling repos only
   through their pinned contracts (`imports/<sibling>/PIN.json`); no relative-path
   imports of sibling internals.
3. No Chromium fork without a dedicated, operator-approved ADR.

## Workflow

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
./scripts/family-invariants.sh
```

- Small, auditable increments; one logical change per commit.
- Conventional commit messages (`feat:`, `fix:`, `docs:`, `chore:`, `test:`, `refactor:`).
- No history rewriting, no force-push, no new remotes or pushes without operator
  authorization.
- Never commit secrets, keys, or real network credentials.

## Documentation

- Architecture changes need an ADR (`docs/adr/`).
- Honest labeling: production-ready / prototype / experimental / mocked / deferred.
