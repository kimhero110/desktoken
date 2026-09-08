# Microsoft detection review draft — QuotaBar v0.4.0-beta.2

Status: authorized by maintainer; not submitted yet. The Software developer flow requires Microsoft account sign-in. No submission ID or vendor determination has been received.

## Suggested submission description

Please review the detection of our public open-source Windows desktop application QuotaBar (repository name: desktoken). We request a determination of the detection cause and whether this is a false positive; we are not assuming the result.

Public release: https://github.com/kimhero110/desktoken/releases/tag/v0.4.0-beta.2

Portable executable SHA-256: `07d9cce61eccc6767620da507cff93f36d438e332c66cda418991921e3f39673`

Installer SHA-256: `814e45d694968104f07f0fbb2b888dc0234718e9ab59c2e1dd66c2167e1d7345`

The archived portable executable was detected and quarantined as `Trojan:Win32/Bearfoos.A!ml` at 2026-09-08T16:02:14Z during a PowerShell Authenticode query. Security intelligence was `1.459.97.0`; engine was `1.1.26080.3`. Separate local download-path events reported `Behavior:Win32/Persistence.A!ml` and a QuotaBar Run-key resource, but those download names alone do not establish the affected version.

With real-time protection enabled and intelligence updated to `1.459.111.0`, a custom scan of the original installer using its native Windows path completed with exit code 0 and “found no threats.” We did not install or execute that installer. The portable executable remains quarantined. We have not disabled protection or restored it to obtain a sample. Please distinguish the portable-file detection from the installer scan result.

The installer has no Authenticode signature. Its SHA-256 matches the public release, and GitHub artifact attestation verification for repository `kimhero110/desktoken` passed. The attestation references source commit `edf4fa2ba63399d961977bea7439e7082b359611`, release workflow `.github/workflows/release.yml`, run `34209580821`, and the two hashes above. This is provenance evidence, not a malware verdict.

The application displays local AI-tool quota and task status. It supports optional user-controlled HKCU Run-key autostart, local task-event hooks, and discovery of the Antigravity language-server process and loopback ports to query quota. Source corrections after beta.2 add a no-console flag to that discovery subprocess and make autostart updates quoted and idempotent. These changes are not included in the submitted beta.2 hashes and are not presented as a confirmed fix for this detection.

We can provide further sanitized evidence on request. The public Windows preview currently carries a detection warning; the stable release assets were retained without claiming antivirus clearance.
