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

# Ensure on specific branch
ensure-branch branch:
	#!/usr/bin/env nu
	let current = (git branch --show-current | str trim)
	if ($current != "{{branch}}") {
		print $"❌ Must be on {{branch}} branch (currently on ($current))"
		exit 1
	}
	print $"✓ On {{branch}} branch"

# Run quick checks before publish
preflight:
	cargo check
	cargo test -q
	cargo package -q

# ── Git Flow Commands ─────────────────────────────────────────────────────────

# Create a new feature branch from dev
feature name:
	#!/usr/bin/env nu
	print $"🌿 Creating feature branch: feature/{{name}}"
	git checkout dev
	git pull origin dev
	git checkout -b $"feature/{{name}}"
	print "✓ Created and switched to feature/{{name}}"
	print "💡 When done: git push -u origin feature/{{name}} and create PR to dev"

# Finish feature: push and show PR creation URL
finish-feature:
	#!/usr/bin/env nu
	let branch = (git branch --show-current | str trim)
	if (not ($branch | str starts-with "feature/")) {
		print "❌ Not on a feature branch"
		exit 1
	}

	print $"🚀 Pushing ($branch)..."
	git push -u origin $branch

	let repo = "graves/awful_aj"  # Update with your repo
	let url = $"https://github.com/($repo)/compare/dev...($branch)?expand=1"
	print ""
	print $"✓ Branch pushed!"
	print $"📝 Create PR: ($url)"

# Sync dev with remote
sync-dev:
	#!/usr/bin/env nu
	print "🔄 Syncing dev branch..."
	git checkout dev
	git pull origin dev
	print "✓ dev branch updated"

# Prepare release: bump version on dev, no publish yet
prepare-release level:
	#!/usr/bin/env nu
	just ensure-branch dev
	just ensure-clean
	just ensure-cargo-edit

	# bump version
	cargo set-version --bump {{level}}

	# read bumped version
	let version = (just version | str trim)
	print $"🔼 Preparing release v($version)"

	# commit version bump
	git add Cargo.toml Cargo.lock
	git commit -m $"Bump version to ($version)"

	# sanity checks
	just preflight

	print ""
	print $"✓ Version bumped to ($version) on dev"
	print "📋 Next steps:"
	print "  1. git push origin dev"
	print "  2. Create PR: dev → main"
	print "  3. Merge PR (triggers GitHub Actions release)"

# Convenience shorthands for prepare-release
prep-patch:
	just prepare-release patch

prep-minor:
	just prepare-release minor

prep-major:
	just prepare-release major

# Publish to crates.io (run after merge to main and GitHub release completes)
publish-crates:
	#!/usr/bin/env nu
	just ensure-branch main
	git pull origin main

	let version = (just version | str trim)
	print $"📦 Publishing v($version) to crates.io..."

	just preflight
	cargo publish --dry-run
	cargo publish

	print $"✓ Published v($version) to crates.io"

# ── Legacy Commands (for backwards compatibility) ─────────────────────────────

# OLD WORKFLOW: Bump version, tag, publish to crates.io, push (deprecated in favor of Git Flow)
release level:
	#!/usr/bin/env nu
	print "⚠️  WARNING: This command is deprecated!"
	print "⚠️  Use the new Git Flow instead:"
	print "    1. just prep-{{level}}      # On dev branch"
	print "    2. Create PR dev → main"
	print "    3. Merge PR (auto-releases via GitHub Actions)"
	print "    4. just publish-crates  # On main branch"
	print ""
	let confirm = (input "Continue with old workflow anyway? (yes/no): ")
	if ($confirm != "yes") {
		exit 0
	}

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
	let current_branch = (git branch --show-current | str trim)
	git push origin $current_branch
	git push origin $"v($version)"

# Convenience shorthands (deprecated)
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

# Rollback the most recent release (delete tag locally, revert version)
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
	print $"   1. Delete tag ($latest_tag) locally"
	print $"   2. Revert Cargo.toml version to ($previous_version)"
	print ""
	let confirm = (input "Continue? (yes/no): ")

	if ($confirm != "yes") {
		print "❌ Rollback cancelled"
		exit 0
	}

	# Delete the tag locally
	print $"🗑️  Deleting local tag ($latest_tag)..."
	git tag -d $latest_tag

	# Set version back to previous version
	just ensure-cargo-edit
	print $"🔽 Setting version back to ($previous_version)..."
	cargo set-version $previous_version

	# Commit the version change
	git add Cargo.toml Cargo.lock
	git commit -m $"Rollback from ($latest_tag) to ($previous_tag)"

	print $"✓ Rollback complete! Version is now ($previous_version)"
	print $"⚠️  To complete rollback:"
	print $"   git push origin --delete ($latest_tag)  # Delete remote tag"
	print $"   git push origin dev --force            # Push rollback commit"
