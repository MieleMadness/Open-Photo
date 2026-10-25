# Contributing to Open-Photo

Thanks for your interest! The project is currently private and still in initial design.

## Development principles

- Prioritize preserving photo data over convenience.
- Keep the desktop application offline-first; network integrations must be opt-in.
- Use test copies, synthetic fixtures, or licensed example photos.
- Document why a new dependency is required and note any licensing or privacy impact.
- Include tests for metadata parsing and every destructive/metadata-writing path.
- Prefer small, reviewable pull requests.

## Proposed workflow

1. Open an issue for nontrivial changes.
2. Work on a feature branch.
3. Run formatting, linting and tests before opening a PR.
4. Explain expected behavior, test coverage, and any compatibility caveats.
5. Do not commit photos containing personal data, API keys, or generated local databases.

## Security concerns

Until a security policy is established, avoid sharing private user images or secrets in public issues. Security reports should include reproduction steps using a synthetic sample wherever possible.
