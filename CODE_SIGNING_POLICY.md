# Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by [SignPath Foundation](https://signpath.org).

## What is signed

Only the Windows release files of Respawn are signed:

- `respawn.exe`, the browser itself
- `Respawn_<version>_x64-setup.exe`, the installer

They are built from this repository by GitHub Actions (`.github/workflows/release.yml`) on
GitHub-hosted runners, from a version tag, and published on the
[Releases](../../releases) page. Nothing built on a personal computer is signed.

## Team roles

| Role | Members |
|---|---|
| Committers and reviewers | [Repository owner](../../) |
| Approvers | [Repository owner](../../) |

Every signing request is approved by hand in SignPath before the files are signed.
All team members use two-factor authentication on GitHub and SignPath.

## Privacy

See [PRIVACY.md](PRIVACY.md). In short: this program will not transfer any information to
other networked systems unless specifically requested by the user or the person installing
or operating it.
