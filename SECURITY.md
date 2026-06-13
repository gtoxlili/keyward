# Security Policy

Keyward is a protocol in draft (`v0`) with no production implementation yet, so today's
"vulnerabilities" are mostly *design* flaws — and those are exactly what I most want to hear about.

The protocol exists to make one promise: the Orchestrator never holds the key. **If you find any
way that promise can be broken** — a path by which the app, its operator, its logs, or a compromise
of it could obtain the credential — please treat it as a security issue, even if it's "just" a
flaw in the spec.

## Reporting

Please report privately rather than opening a public issue:

- **GitHub** — use private vulnerability reporting ("Report a vulnerability" under the repo's
  Security tab). Preferred.
- **Email** — gtoxlili@outlook.com

## Scope

- **In scope:** the protocol design and the spec; and, once they exist, the reference Executor and
  Orchestrator SDK.
- **Out of scope for now:** there is no deployed service to attack — this is a spec and a design
  note.

## What to expect

This is a personal project, so I can't offer a formal SLA, but I'll acknowledge a report as quickly
as I reasonably can, and I'll credit you when a fix or spec change lands, unless you'd rather stay
anonymous.
