# Contributing to Keyward

Keyward is early. There's a design note ([README](./README.md)), a draft spec
([spec.md](./docs/spec.md)), and a Rust reference implementation (see
[implementation.md](./docs/implementation.md)). It's `v0` and unstable, which shapes what's
actually useful to contribute.

## The most useful thing you can do

Argue with the spec. The **Open questions** section of the spec is where I'm least sure of myself,
and the threat model in the README is the thing I most want stress-tested. Open an issue if:

- You can break the core promise — any path by which the Node (or its operator, its logs,
  or a breach of it) could still end up with the key. That's not a typo, it's a flaw in the *idea*,
  and I want to know. (If it's that, please read [SECURITY.md](./SECURITY.md) first and report it
  privately.)
- One of the open questions has an obvious answer I'm missing.
- A role, message, or policy field is ambiguous, or wouldn't survive contact with a real provider.
- You're building something that would use this and it doesn't fit your case.

Prose issues are welcome. You do not need to bring a PR.

## If you do want to send a PR

- For spec changes, explain the reasoning in the PR description, not just the diff. v0 stays marked
  unstable.
- Keep things transport- and provider-agnostic unless you're explicitly adding an adapter.
- One idea per PR — easier to discuss and to say yes to.

## What's already decided

The license is [Apache-2.0](./LICENSE). Beyond that, treat everything in v0 as provisional —
nothing is load-bearing until v1.

## Be decent

By participating you agree to the [Code of Conduct](./CODE_OF_CONDUCT.md). Assume good faith,
disagree about the work and not the person.
