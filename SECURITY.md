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

- `sysmedic-fix-helper` and `sysmedic-fix-helper-destructive` are the only
  components that run as root. Each is launched through **pkexec**/**polkit**
  under its own action, accepts a fix **id** only — never a command or a
  serialized plan — and rebuilds the plan from a compiled-in registry, so a
  compromised unprivileged caller cannot inject commands. Each also refuses
  the other tier's ids.
- A helper collects **only the snapshot sections its fix declares**
  (`Fix::needs_collectors`), so applying one fix does not run sixteen external
  tools as root.
- `undo` likewise rebuilds its commands from the registry keyed on the stored
  fix id, not from the commands recorded in the journal file. The two values
  that do come from the journal — snapd's previous `refresh.retain` and the
  ufw rule a firewall enable added — are validated (an integer in snapd's
  range; a `<port>/tcp` spec with the port in range) before a root process
  substitutes them into an argument.
- The transaction journal (`/var/lib/sysmedic/journal.json`) is written
  `0644` in a `0755` directory, both owned by root, via an atomic,
  symlink-safe rename. What it needs is **integrity, not secrecy**: only root
  can write it, its contents are fix ids and timestamps, and the unprivileged
  GUI and CLI must be able to read it — otherwise `sysmedic undo` previews
  "nothing to undo" for fixes that are sitting in it.

Reports that concern this path, privilege escalation, or the polkit policy are
highest priority.

## Supported versions

SysMedic is pre-1.0; security fixes land on the latest `main` and the most
recent release.
