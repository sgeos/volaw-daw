# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| Unreleased, the default branch | Yes |

Replace this table at the first release, and keep it current with each release line.

## Reporting a vulnerability

Please report security issues privately, so that a fix can be prepared before public
disclosure. Open a draft advisory at
<https://github.com/sgeos/volaw-daw/security/advisories/new>. The maintainer is
notified and can work on the fix with you in the same place.

Please include the affected version or commit, a minimal way to reproduce the issue, the
expected and observed behaviour, and your assessment of severity if you have one.

## Response

The maintainer aims to acknowledge a report within a few days, and to either confirm the
issue or explain why it is not a vulnerability. Confirmed issues are fixed in a release
whose disclosure timeline is agreed with the reporter.

## Scope

The trust boundaries are the files the composer reads. Project files in TOML,
Standard MIDI Files and Keleusma scripts are untrusted input. A malformed file should
produce an error rather than a crash, unbounded memory use or a hang. Keleusma scripts
run in the Keleusma virtual machine with an arena bounded at load time and a bounded
number of resumptions per tick.

Out of scope are vulnerabilities in SDL3, SDL3_ttf and Keleusma themselves, which
should be reported to those projects, and denial of service by a user against their
own local session.
