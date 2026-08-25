# Host safety policy for autonomous development

**Authority:** normative for automated/agent work on a developer machine.

521C is allowed to build, test and interact with Bluetooth hardware, but the developer workstation is not a disposable sandbox. The agent has broad authority inside this repository and deliberately narrow authority outside it.

## 1. Default boundary

The working repository and its normal build/cache directories are the default write boundary.

Allowed without asking:

- create/edit/delete files inside this repository, excluding secrets that should not be committed;
- use normal project-local dependency managers and build tools;
- create temporary files under normal OS temp locations;
- run tests, builds, linters, formatters, local development servers and package builders;
- inspect system information needed for compatibility diagnosis;
- read BlueZ/PipeWire/MPRIS state through documented user/system APIs;
- start/stop processes launched by the agent for this project;
- create local git branches, commits and tags for this repository;
- use the provided GitHub credentials only for this repository's issues, branches, pull requests, CI and releases.

## 2. Dependency installation

Prefer, in order:

1. repository-local dependencies and lockfiles;
2. user-scoped toolchains (`rustup`, user-local binaries, language package managers);
3. distro packages from official Linux Mint/Ubuntu repositories when native build/runtime libraries are required.

A narrowly scoped package installation is allowed when it is necessary to build/test 521C and the package purpose is understood. Examples include compiler/build tooling and development libraries required by Slint, AppImage tooling, D-Bus, BlueZ, or PipeWire integration.

Do not:

- run broad `apt upgrade`, `full-upgrade`, distribution upgrades, or repository migrations;
- remove existing system packages to resolve a dependency conflict;
- add third-party APT repositories, PPAs, curl-pipe-root installers, or unsigned package sources merely for convenience;
- replace the user's Node, Python, Rust, desktop, audio, Bluetooth, browser, or shell setup globally when a user/project-scoped alternative exists.

If root is required for a narrowly scoped official distro package install, minimize the command to the exact packages required and do not combine it with unrelated system changes.

## 3. Forbidden host mutations

Never autonomously:

- recursively delete or overwrite `/`, `/home`, the user's home directory, `/etc`, `/usr`, `/var`, `/boot`, mounted disks, or unrelated repositories;
- run destructive disk/filesystem commands (`mkfs`, partitioning, raw disk writes, mass `rm -rf`, destructive `find -delete`) outside an explicitly created project temp directory;
- change firewall policy, SSH configuration, login/session policy, bootloader, kernel parameters, user accounts, passwords, sudoers, SELinux/AppArmor policy, or full-disk settings;
- disable security tooling or OS protections;
- replace, mask, disable, or globally reconfigure BlueZ, PipeWire, WirePlumber, NetworkManager, systemd, or desktop services;
- install persistent background daemons unrelated to 521C;
- expose development servers on `0.0.0.0` by default;
- copy, inspect, upload, or modify unrelated personal files, browser profiles, SSH keys, cloud credentials, password stores, or other repositories;
- commit credentials, API keys, Bluetooth identifiers that the privacy policy excludes, or private user data.

## 4. Process and service interaction

It is acceptable to query service state and to restart a project-owned process.

For system services such as Bluetooth/audio, prefer observation and graceful reconnect logic. A one-time restart of an existing service should be treated as exceptional debugging, not as part of normal application operation. Do not make permanent service-unit changes to make 521C work.

521C itself must run without root in normal use.

## 5. Bluetooth hardware safety

Read-only discovery, service enumeration, characteristic inspection, notification subscription and known-safe reads are allowed when hardware is available.

No real-device write may occur until the repository has a central write-authorization policy satisfying issue #1 and sufficient evidence/provenance for that operation satisfying issue #6.

Even after those gates:

- unknown/generic devices remain read-only;
- only proven, allowlisted, reversible HT08 writes may be automated;
- experimental writes require the product's explicit session opt-in mechanism;
- `0x01` reset defaults, `0x02` clear pairing and `0x03` factory reset are forbidden to unattended automation at every layer;
- firmware OTA is forbidden until a separately proven safe design exists;
- Find Earbuds/chime requires interactive preflight and must never be fired by unattended automation;
- do not probe unknown opcodes against hardware to discover their effect.

A failed or ambiguous real-device operation is evidence to investigate, not permission to send increasingly speculative commands.

## 6. Network and credentials

Network access is allowed for:

- authoritative technical documentation;
- package registries used by the declared toolchain;
- GitHub operations scoped to `pedro-labsabs/521C`;
- release/download verification required for normal build tooling.

Do not upload repository code, logs, captures, device identifiers or user data to arbitrary third-party services as a debugging shortcut.

Credentials supplied to the agent are operational secrets. Use them only for the intended provider/repository and never print or commit their values.

## 7. Recovery rule

Before a risky-but-allowed repository migration or architecture change, create a git checkpoint first. Prefer reversible changes and normal version-control recovery over manual destructive cleanup.

If a required action falls outside this policy, do not reinterpret broad project autonomy as host authorization. Find a safer implementation path; only if none exists is user intervention justified.
