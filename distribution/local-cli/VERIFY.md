# Verify a local release candidate

> Status: Proposed G1b/G8-local procedure under issue #186 and Decision 0036.
> A candidate is not a published, supported, or generally available release.

## 1. Verify candidate contents offline

Use the exact repository revision that produced the candidate and run:

```text
cargo run --locked -p xtask -- verify-local-release-candidate \
  --candidate <digest-named-candidate-directory>
```

Success emits one canonical
`codenoesis.local-release-candidate-verification/v1` document. The command
revalidates the candidate tree, manifest, checksums, ZIP records, embedded G1a
bundle, source/target/lock identity, SBOM, license, advisory, and unsafe
evidence. It is read-only and does not verify a cryptographic signer.

## 2. Verify signer and provenance

The downloaded workflow artifact places `attestation.jsonl` beside the
immutable candidate directory. Verify the ZIP subject with GitHub CLI:

```text
gh attestation verify \
  <candidate-directory>/<candidate-archive.zip> \
  --bundle <attestation.jsonl> \
  --repo smutti/codenoesis \
  --signer-workflow smutti/codenoesis/.github/workflows/local-release-candidate.yml \
  --source-ref refs/heads/main \
  --deny-self-hosted-runners
```

Repeat the command for `manifest.json`, `checksums.sha256`, and every file below
`evidence/`. Verification must fail after changing a subject byte, bundle,
repository, signer workflow, source ref, or digest.

## Trust boundary

The cryptographic result establishes that GitHub's hosted OIDC identity for the
named protected-main workflow attested the exact subject digest with the SLSA
provenance predicate. It does not establish product correctness, support, a
vulnerability-response commitment, durable publication, or GA. Those require
independent acceptance of the complete release evidence and a later G9
decision.
