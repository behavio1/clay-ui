# Releasing Clay UI

## First publication: owner bootstrap

No package exists yet. npm's package-level Trusted Publisher settings require an existing package. The workflow cannot bootstrap package ownership through OIDC.

1. Confirm ownership/availability of `clayui` and each platform name in `package.json`. Use your npm account with 2FA; run `npm login` interactively on your own machine. Do not create or add an npm token to Actions.
2. Review source and notices. Run the release checks, build all targets on their matching systems with path remapping, and generate the native packages with `scripts/build-platform-package.mjs`. Keep executable modes 755. Run `scripts/packed-smoke.mjs` on every matching platform. Collect the reviewed native package folders in `dist-npm`.
3. Manually publish the actual first version (currently 0.1.0) with your interactive account: run `npm publish ./dist-npm/clayui-<platform> --access public` for darwin-arm64, darwin-x64, linux-x64, linux-arm64 and win32-x64. Confirm each with `npm view clayui-<platform>@0.1.0 version`. Only then run `npm publish --access public` from the launcher root. These manual publications do not have workflow provenance; do not claim they do. Never publish placeholders under this version.
4. In npmjs.com, open **each** of these packages → Settings → Trusted Publisher → GitHub Actions. Set organization/user `behavio1`, repository `clay-ui`, workflow filename `release.yml`, no environment name. Allow direct `npm publish`. The public repository metadata must match this repository. Confirm all package settings and, after OIDC verification, disallow token publishing in the package settings. There is no workflow npm token or NODE_AUTH_TOKEN.
5. The first OIDC release must use a **new version** (for example 0.1.1): update Cargo.toml, Cargo.lock, package.json and every optionalDependency together, review and tag that version. Do not rerun a tag for the already manually published version: npm versions are immutable. The workflow uses GitHub-hosted runners, Node 24.22.1 and npm 11.20.0 (OIDC requires Node ≥22.14.0 and npm ≥11.5.1), grants `id-token: write` only to publish, publishes all native packages before the launcher and requests provenance.
6. Verify every package version and provenance in npm, then test public `npx clayui@<version> --version` and a sandboxed `init --target agents --no-sync` on the supported systems. Public registry resolution and account settings require the owner's release; local tarballs cannot prove them.

If publication stops after some native packages, do not publish the launcher until all its exact optionalDependency versions are available. Inspect registry state and finish the missing packages manually through the configured trusted workflow or prepare a new coordinated version; never overwrite an existing version.

## Local acceptance

Run `cargo test --locked`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo fmt --check`, `npm run test:node`, `cargo build --locked`, `npm run parity`, and `python3 scripts/third-party-notices.py --check`.

Build release binaries with `RUSTFLAGS="--remap-path-prefix=$HOME=/build --remap-path-prefix=$PWD=/clayui" cargo build --locked --release`. The packed smoke test scans the actual binary for `/Users/` and test transport overrides, packs both tarballs, and clean-installs them through the launcher in a temporary HOME/cwd. Windows CI additionally uses static CRT; its Rust integration coverage excludes only Unix symlink tests. `e2e/live.mjs` exercises the native release CLI against current upstreams; it requires network access and is separate from deterministic parity.

Action pins were verified against the public action repositories. Reverify pins when updating them. The retained `.mjs` reference implementation is development-only and supports the Node suite and parity oracle; it is not packed into the product.

References: [npm trusted publishing](https://docs.npmjs.com/trusted-publishers/), [GitHub hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
