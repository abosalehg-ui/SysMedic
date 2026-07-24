# Security Policy

SysMedic runs privileged system fixes, so we take security reports seriously.

## Reporting a vulnerability

Please report suspected vulnerabilities privately rather than opening a public
issue:

- Use GitHub's **“Report a vulnerability”** button under the repository’s
  **Security** tab (Private Vulnerability Reporting), or
- email the maintainer at **ar0.history@gmail.com** with the details.

Please include: affected version/commit, a description of the issue, and — if
possible — a proof of concept and the impact you expect. We aim to acknowledge
reports within a few days.

## Scope and trust model

The security-critical surface is the privileged path:

- `sysmedic-fix-helper` (crate `sysmedic-daemon`) is the only component that
  runs as root. It is launched through **pkexec**/**polkit** and accepts a
  fix **id** only — never a command or a serialized plan — and rebuilds the
  plan from a compiled-in registry, so a compromised unprivileged caller
  cannot inject commands.
- `undo` likewise rebuilds its commands from the registry keyed on the stored
  fix id, not from the commands recorded in the journal file.
- The transaction journal (`/var/lib/sysmedic/journal.json`) is written
  `0600` in a `0700` directory via an atomic, symlink-safe rename.

Reports that concern this path, privilege escalation, or the polkit policy are
highest priority.

## Supported versions

SysMedic is pre-1.0; security fixes land on the latest `main` and the most
recent release.
