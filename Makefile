.PHONY: version dev build-mac build build-native install sign test notices clean

# Version format: YYYY.MMDD.N (semver-compatible)
# e.g., 2026.601.0 = first change committed on June 1, 2026
#       2026.601.1 = second change committed that day
#       2026.1225.0 = first change committed on Dec 25
#
# The version is a pure function of git history, NOT wall-clock build time,
# computed by scripts/compute-version.sh (the same script the pre-commit hook
# uses, so the build and the committed package.json always agree). Here we use
# its default mode = the version of HEAD as committed, making builds fully
# reproducible: the same commit always yields the same version.
#
# The pre-commit hook keeps package.json in sync in-repo; the build also passes
# the computed version to the Tauri CLI as a config override, so a commit that
# skipped the hook (--no-verify) still ships the correct version.
VERSION := $(shell sh scripts/compute-version.sh)
TAURI_VERSION_OVERRIDE := --config '{"version":"$(VERSION)"}'
MAC_BUNDLE_DIR := src-tauri/target/universal-apple-darwin/release/bundle

version:
	@echo "$(VERSION)"

# Run the app in development mode (Vite dev server + the Tauri window; the
# renderer hot-reloads, the Rust side rebuilds on change)
dev:
	npm run dev

# Universal (arm64 + x86_64) .app and .dmg
build-mac:
	npm ci
	rustup target add aarch64-apple-darwin x86_64-apple-darwin
	npm run build:mac -- $(TAURI_VERSION_OVERRIDE)

build: build-mac

# Native-architecture build of every bundle the host platform supports (faster
# than universal; bundles land in src-tauri/target/release/bundle/).
build-native:
	npm run tauri -- build $(TAURI_VERSION_OVERRIDE)

install:
	open "$$(ls -t $(MAC_BUNDLE_DIR)/dmg/*.dmg src-tauri/target/release/bundle/dmg/*.dmg 2>/dev/null | head -1)"

# App display name, sourced from package.json productName (single source of truth)
APP_NAME := $(shell node -p "require('./package.json').productName")

# Sign the installed app on this Mac (required after every install: the app is self-signed). Signs
# the same way the bundler does (ad-hoc, hardened runtime, the app's entitlements).
sign:
	sudo codesign --force --deep --options runtime --entitlements src-tauri/app/entitlements.mac.plist --sign - "/Applications/$(APP_NAME).app"

# Rust + JS unit tests (what CI runs).
test:
	cd src-tauri && cargo test --workspace
	npm test

# Regenerate notices/rust.md (cargo-about) and notices/frontend.md (npm deps).
notices:
	sh scripts/notices/generate.sh

# Remove generated build artifacts (safe: never touches tracked source like
# build/ or local files like .env / node_modules)
clean:
	rm -rf dist out coverage test-outputs ash_output
