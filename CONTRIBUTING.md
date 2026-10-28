# Contributing

Contributions are welcome, including fixes, documentation and tests.

**The project is licensed under 0BSD.** By contributing you agree that your contribution
is released under the same licence. Do not contribute material that you did not write,
or that carries terms of its own.

Participation in the project's spaces follows the
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). Contributions are judged on their technical
merits.

## Before opening a pull request

1. Run `./scripts/check.sh` and confirm that it exits zero.
2. Keep each pull request to one change, on a branch named
   `<scope>/<short-description>`, such as `fix/parser-error-recovery`.
3. Write commit messages as conventional commits, such as `feat:`, `fix:`, `docs:`,
   `test:` or `chore:`.
4. Add an entry under `[Unreleased]` in `CHANGELOG.md` for any change a user would notice.
5. Do not weaken a check to make it pass. If a check is wrong, say so in the pull request
   and fix it deliberately.

## Reporting security issues

Do not open a public issue for a vulnerability. Follow [`SECURITY.md`](SECURITY.md).
