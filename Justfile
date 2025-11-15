# Run all recipes with Nushell
set shell := ["nu", "-c"]

# ── helpers ───────────────────────────────────────────────────────────────────

# Print current crate version (as seen by Cargo)
version:
	#!/usr/bin/env nu
	cargo metadata --no-deps --format-version 1
	| from json
	| get packages.0.version
	| str trim
	| print $in

# Ensure no uncommitted changes unless ALLOW_DIRTY=true
ensure-clean:
	#!/usr/bin/env nu
	if (($env.ALLOW_DIRTY? | default "false" | str downcase) == "true") {
		print "⚠️  ALLOW_DIRTY=true: skipping clean-tree check"
	} else {
		if (git status --porcelain | is-empty) {
			print "✓ Working tree clean"
		} else { print "❌ Working tree not clean. Commit or set ALLOW_DIRTY=true"; exit 1 }
	}

# Ensure cargo-edit is present (for cargo set-version)
ensure-cargo-edit:
	#!/usr/bin/env nu
	if (which cargo-set-version | is-empty) {
		cargo install cargo-edit --quiet --force
	}

# Run quick checks before publish
preflight:
	cargo check
	cargo test -q
	cargo package -q

# ── main entrypoints ──────────────────────────────────────────────────────────

# Bump {patch|minor|major|<semver>}, commit, tag, publish to crates.io, push to GitHub
# Usage:
#   just release patch
#   just release minor
#   just release major
release level:
	#!/usr/bin/env nu
	just ensure-clean
	just ensure-cargo-edit

	# bump version
	cargo set-version --bump {{level}}

	# read bumped version
	let version = (just version | str trim)
	print $"🔼 Bumping to v($version)"

	# commit & tag
	git add Cargo.toml Cargo.lock
	git commit -m $"Release v($version)"
	git tag $"v($version)"

	# sanity checks (build/tests/package), then a dry-run publish
	just preflight
	print "🚀 Dry run publish…"
	cargo publish --dry-run

	# real publish
	print "📦 Publishing v($version) to crates.io…"
	cargo publish

	# push branch & tag
	print "⬆️  Pushing to GitHub…"
	git push origin main
	git push origin $"v($version)"

# Convenience shorthands
patch:
	just release patch

minor:
	just release minor

major:
	just release major

# Publish the current version without bumping
publish-only:
	just ensure-clean
	just preflight
	cargo publish

# Rollback the most recent release (delete tag, revert version in Cargo.toml)
# WARNING: This will delete the most recent tag locally and remotely
rollback:
	#!/usr/bin/env nu
	# Get the most recent tag
	let latest_tag = (git describe --tags --abbrev=0 | str trim)
	print $"📌 Most recent tag: ($latest_tag)"

	# Get the previous tag (second most recent)
	let previous_tag = (git describe --tags --abbrev=0 ($latest_tag + "^") | str trim)
	print $"📌 Previous tag: ($previous_tag)"

	# Extract version from previous tag (remove 'v' prefix)
	let previous_version = ($previous_tag | str replace 'v' '')

	# Confirm with user
	print $"⚠️  This will:"
	print $"   1. Delete tag ($latest_tag) locally and remotely"
	print $"   2. Revert Cargo.toml version to ($previous_version)"
	print $"   3. Reset HEAD to the commit before ($latest_tag)"
	print ""
	let confirm = (input "Continue? (yes/no): ")

	if ($confirm != "yes") {
		print "❌ Rollback cancelled"
		exit 0
	}

	# Delete the tag locally
	print $"🗑️  Deleting local tag ($latest_tag)..."
	git tag -d $latest_tag

	# Delete the tag remotely
	print $"🗑️  Deleting remote tag ($latest_tag)..."
	git push origin --delete $latest_tag

	# Reset to the commit before the latest tag
	print $"⏮️  Resetting to commit before ($latest_tag)..."
	git reset --hard HEAD~1

	# Set version back to previous version
	just ensure-cargo-edit
	print $"🔽 Setting version back to ($previous_version)..."
	cargo set-version $previous_version

	# Commit the version change
	git add Cargo.toml Cargo.lock
	git commit -m $"Rollback from ($latest_tag) to ($previous_tag)"

	print $"✓ Rollback complete! Version is now ($previous_version)"
	print $"⚠️  Don't forget to push: git push origin main --force"
