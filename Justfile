# Run all recipes with Nushell
set shell := ["nu", "-c"]

# --- helpers ---------------------------------------------------------------

# Print current crate version (as seen by Cargo)
version:
    cargo metadata --no-deps --format-version 1 \
    | from json \
    | get packages.0.version \
    | str trim \
    | echo $in

# Ensure no uncommitted changes unless ALLOW_DIRTY=true
ensure-clean:
    if (("AL" + "LOW_DIRTY" | str downcase) in (env | get | get name)) and ($env.ALLOW_DIRTY == "true") {
        echo "⚠️  ALLOW_DIRTY=true: skipping clean-tree check"
    } else {
        if (git status --porcelain | is-empty) {
            echo "✓ Working tree clean"
        } else {
            echo "❌ Working tree not clean. Commit or set ALLOW_DIRTY=true"; exit 1
        }
    }

# Ensure cargo-edit is present (for cargo set-version)
ensure-cargo-edit:
    if (which cargo-set-version | is-empty) {
        cargo install cargo-edit --quiet --force
    }

# Run quick checks before publish
preflight:
    cargo check
    cargo test -q
    cargo package -q   # verifies package manifests and include/exclude rules

# --- main entrypoints ------------------------------------------------------

# Bump {patch|minor|major|<semver>}, commit, tag, publish to crates.io, push to GitHub
# Usage:
#   just release patch
#   just release minor
#   just release major
release level:
    just ensure-clean
    just ensure-cargo-edit

    # bump version
    cargo set-version --bump {{level}}

    # read bumped version
    let version = (just version | str trim)
    echo $"🔼 Bumping to v($version)"

    # commit & tag
    git add Cargo.toml Cargo.lock
    git commit -m $"Release v($version)"
    git tag $"v($version)"

    # sanity checks (build/tests/package), then a dry run publish for fast fail
    just preflight
    echo "🚀 Dry run publish…"
    cargo publish --dry-run

    # real publish
    echo "📦 Publishing v($version) to crates.io…"
    cargo publish

    # push branch & tag
    echo "⬆️  Pushing to GitHub…"
    git push origin main
    git push origin $"v($version)"

# Convenience shorthands
patch:
    just release patch

minor:
    just release minor

major:
    just release major

# If you want to publish the *current* version without bumping:
publish-only:
    just ensure-clean
    just preflight
    cargo publish
