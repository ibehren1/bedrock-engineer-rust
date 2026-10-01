# Release Guide

Releases are cut automatically: every push to `main` (including PR merges) runs
`.github/workflows/build-and-release.yml`, which builds the app, then publishes a GitHub release
tagged `v<version>` whose body is the newest dated section of `CHANGELOG.md`. There is no release
branch and no manual version bump.

## Versioning

The version is **`YYYY.MMDD.N`**, derived from git history by `scripts/compute-version.sh`:
the commit date, then the 0-based index of the commit among that day's commits
(`2026.930.0` is the first commit on 30 Sep 2026, `2026.930.1` the second, `2026.1225.0` the
first on 25 Dec). It is a pure function of git history, so the same commit always gets the same
version.

One source of truth feeds everything:

| Consumer | How it gets the version |
| -------- | ----------------------- |
| `package.json` `version` | Stamped by the pre-commit hook (`scripts/hooks/pre-commit`, installed by `npm install`) using `compute-version.sh --pending`. |
| Tauri app, bundles, About/`getVersion()` | `src-tauri/app/tauri.conf.json` has `"version": "../../package.json"`, so Tauri reads `package.json`. The runtime value is `app.package_info().version` (renderer: `getVersion()` from `@tauri-apps/api/app`, allowed by `core:default`). |
| Release tag | `v` + `package.json` version, read by the release job. |
| Local `make` builds | Recompute from HEAD and override with `--config '{"version":"…"}'`, so a commit made with `--no-verify` still builds with the right version. |

The Cargo `[workspace.package] version` in `src-tauri/Cargo.toml` is **not** used for anything
user-visible (Tauri prefers the config version); it only names the crates in `Cargo.lock`.

### Platform version limits

`YYYY.MMDD.N` is valid semver, and works for every bundle we ship:

- **macOS** `CFBundleShortVersionString` / `CFBundleVersion`: any dotted numeric string.
- **Windows NSIS** and the PE version resource: four 16-bit fields (max 65535 each);
  `2026.1231.N` fits.
- **Linux deb / AppImage**: free-form.

**Windows MSI is not built.** MSI `ProductVersion` caps the major and minor fields at 255, so
`2026.930.N` is rejected. The earlier Electron build only shipped NSIS too. If an
MSI is ever needed, set `bundle.windows.wix.version` to a mapped value, for example
`<YY>.<M>.<DD*1000+N>` (`2026.930.4` becomes `26.9.30004`), which stays monotonic while N < 1000.

## What gets built

| Platform | Command | Bundles |
| -------- | ------- | ------- |
| macOS | `npm run build:mac` | universal `.app` + `.dmg` |
| Windows | `npm run build:win` | NSIS `-setup.exe` |
| Linux | `npm run build:linux` | AppImage + deb |

Bundles land in `src-tauri/target/<target>/release/bundle/<kind>/` (native builds: in
`src-tauri/target/release/bundle/`). CI renames them to `bedrock-engineer-rust-<version>-<arch>.<ext>`
before attaching them to the release.

The Electron build this app replaced also shipped a macOS `.pkg` and a Linux snap; Tauri has no
such targets, so releases no longer include them.

## Local builds

```bash
make version            # print the version HEAD builds as
make dev                # Vite dev server + the app window
make build-mac          # universal .app/.dmg (adds the x86_64 Rust target if missing)
make build-native       # native arch only (faster)
make install            # open the newest .dmg
make sign               # ad-hoc re-sign /Applications/Bedrock Engineer.app
make test               # cargo test --workspace + npm test
make notices            # regenerate notices/rust.md and notices/frontend.md
```

## Code signing

- **macOS:** builds are ad-hoc signed (`-`) and not notarized by default. Set the standard Tauri CLI environment variables to sign and notarize with a real identity:
  `APPLE_SIGNING_IDENTITY`, `APPLE_CERTIFICATE` (base64 .p12) + `APPLE_CERTIFICATE_PASSWORD`,
  and `APPLE_ID` + `APPLE_PASSWORD` + `APPLE_TEAM_ID` for notarization. In CI these come from
  repository secrets of the same names; unset secrets are skipped. Hardened-runtime entitlements
  are in `src-tauri/app/entitlements.mac.plist`.
- **Windows:** unsigned. Tauri supports `bundle.windows.certificateThumbprint`
  or `bundle.windows.signCommand` if signing is added later.

## Third-party notices

`notices/rust.md` (cargo-about, config in `src-tauri/about.toml` + `about.hbs`) and
`notices/frontend.md` (`scripts/notices/frontend-notices.mjs`, from the npm production tree)
are bundled into the app under `notices/`, together with `LICENSE` and `NOTICE`. CI
regenerates both before bundling; the step fails if a dependency's license is outside the
accept-lists, so a new copyleft or unknown license is caught before it ships. Commit the
regenerated files when dependencies change.

Install cargo-about locally with `cargo install cargo-about --locked --features cli`.

## Troubleshooting

- **Tag already exists:** the release job skips publishing if `v<version>` exists. That
  happens only when `package.json` was not re-stamped (commit made with `--no-verify`); the
  next normal commit fixes it.
- **Build failed:** the release still publishes (notes-only or with whatever installers built).
  Fix on `main` and push again; the next commit gets a new version.
- **`failed to remove extra attributes from app bundle: failed to run xattr` (local macOS):**
  the bundler runs `xattr -crs` on the `.app`, and endpoint DLP agents on managed Macs (for
  example Next DLP's `com.nextdlp.reveal.*` attributes) tag files in the working tree with
  attributes that cannot be removed. A plain copy carries them into the bundle. To avoid that,
  `src-tauri/app/build.rs` stages the bundle resources by content (no attributes) into
  `src-tauri/app/bundle-resources/`, and `bundle.resources` points there. If the error comes back,
  check that every entry in `bundle.resources` points into `bundle-resources/` and is listed in
  `RESOURCES` in `build.rs`.
- **Notices step failed:** read its log for the offending crate or package, review its license,
  then either add it to the accept-list / a per-crate exception in `src-tauri/about.toml`, or to
  `KNOWN_EXCEPTIONS` in `scripts/notices/frontend-notices.mjs`.
