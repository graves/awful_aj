# Release Process for awful_aj

## Quick Reference

### Standard Release Flow

```bash
# 1. Run your existing Justfile release command (bumps version, tags, publishes to crates.io)
just release patch   # or: minor, major

# 2. GitHub Actions automatically:
#    - Detects the pushed tag (v*)
#    - Builds binaries for all platforms
#    - Creates GitHub Release
#    - Uploads signed binaries (if signing configured)
#    - Uploads SHA256 checksums
```

That's it! The GitHub Actions workflow handles everything after you push the tag.

---

## Supported Platforms

The workflow builds for **6 platforms** with **Debian packages** for Linux:

| Platform | Target | Architecture | Formats |
|----------|--------|--------------|---------|
| Linux | `x86_64-unknown-linux-gnu` | x86_64 | tar.gz + .deb |
| Linux | `aarch64-unknown-linux-gnu` | ARM64 | tar.gz + .deb |
| Windows | `x86_64-pc-windows-gnu` | x86_64 | zip |
| Windows | `aarch64-pc-windows-msvc` | ARM64 | zip |
| macOS | `x86_64-apple-darwin` | Intel | tar.gz |
| macOS | `aarch64-apple-darwin` | Apple Silicon | tar.gz |

---

## Release Artifacts

Each release includes:

### Binaries (tar.gz/zip)
```
awful_aj-v0.3.11-x86_64-unknown-linux-gnu.tar.gz
awful_aj-v0.3.11-aarch64-unknown-linux-gnu.tar.gz
awful_aj-v0.3.11-x86_64-pc-windows-gnu.zip
awful_aj-v0.3.11-aarch64-pc-windows-msvc.zip
awful_aj-v0.3.11-x86_64-apple-darwin.tar.gz
awful_aj-v0.3.11-aarch64-apple-darwin.tar.gz
```

### Debian Packages
```
awful-aj_v0.3.11_amd64.deb
awful-aj_v0.3.11_arm64.deb
```

### Checksums
```
awful_aj-v0.3.11-x86_64-unknown-linux-gnu.tar.gz.sha256
awful_aj-v0.3.11-aarch64-unknown-linux-gnu.tar.gz.sha256
awful_aj-v0.3.11-x86_64-pc-windows-gnu.zip.sha256
awful_aj-v0.3.11-aarch64-pc-windows-msvc.zip.sha256
awful_aj-v0.3.11-x86_64-apple-darwin.tar.gz.sha256
awful_aj-v0.3.11-aarch64-apple-darwin.tar.gz.sha256
awful-aj_v0.3.11_amd64.deb.sha256
awful-aj_v0.3.11_arm64.deb.sha256
```

Each tar.gz/zip archive contains:
- Binary (`aj` or `aj.exe`)
- `README.md`
- `LICENSE`
- `templates/` directory with YAML templates

Debian packages install to standard system locations. See [DEBIAN.md](DEBIAN.md) for details.

---

## Detailed Release Steps

### 1. Prepare Release

Ensure everything is ready:
```bash
# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run clippy
cargo clippy

# Verify clean working tree
git status
```

### 2. Create Release with Justfile

```bash
# Patch release (0.3.11 → 0.3.12)
just release patch

# Minor release (0.3.11 → 0.4.0)
just release minor

# Major release (0.3.11 → 1.0.0)
just release major
```

This command:
1. ✓ Ensures working tree is clean
2. ✓ Bumps version in Cargo.toml
3. ✓ Commits version bump
4. ✓ Creates git tag (e.g., `v0.3.12`)
5. ✓ Runs preflight checks (build, test, package)
6. ✓ Publishes to crates.io
7. ✓ Pushes to GitHub (main branch + tag)

### 3. Monitor GitHub Actions

After pushing the tag:

1. Go to: https://github.com/graves/awful_aj/actions
2. Find the "Release" workflow run
3. Monitor the build matrix (6 parallel builds)
4. Check for any failures

### 4. Verify Release

Once complete:

1. Go to: https://github.com/graves/awful_aj/releases
2. Find your release (e.g., "Release v0.3.12")
3. Verify all 12 files are attached (6 archives + 6 checksums)
4. Download and test a binary:

```bash
# Linux/macOS
curl -LO https://github.com/graves/awful_aj/releases/download/v0.3.12/awful_aj-v0.3.12-x86_64-unknown-linux-gnu.tar.gz
tar -xzf awful_aj-v0.3.12-x86_64-unknown-linux-gnu.tar.gz
cd awful_aj-v0.3.12-x86_64-unknown-linux-gnu
./aj --version

# Verify checksum
curl -LO https://github.com/graves/awful_aj/releases/download/v0.3.12/awful_aj-v0.3.12-x86_64-unknown-linux-gnu.tar.gz.sha256
shasum -a 256 -c awful_aj-v0.3.12-x86_64-unknown-linux-gnu.tar.gz.sha256
```

---

## Troubleshooting

### Build Failures

**Cross-compilation errors:**
- Check [Cross.toml](Cross.toml) for correct Docker images
- Ensure all targets are in [rust-toolchain.toml](rust-toolchain.toml)
- Check dependency compatibility with target platform

**Timeout errors:**
- Builds usually take 15-30 minutes total
- Increase timeout in workflow if needed

### Release Creation Fails

**"Tag already exists":**
```bash
# Delete tag locally and remotely
git tag -d v0.3.12
git push origin --delete v0.3.12

# Re-run: just release patch
```

**"Permission denied":**
- Ensure GitHub Actions has write permissions
- Go to: Settings → Actions → General → Workflow permissions
- Select "Read and write permissions"

### Signing Issues

See [SIGNING.md](SIGNING.md) for detailed signing setup and troubleshooting.

If signing is not configured, binaries will be **unsigned** (which is fine for most users).

---

## Manual Release (Emergency)

If you need to create a release manually:

### 1. Build All Targets Locally

```bash
# Install cross
cargo install cross --git https://github.com/cross-rs/cross

# Build all targets
targets=(
  "x86_64-unknown-linux-gnu"
  "aarch64-unknown-linux-gnu"
  "x86_64-pc-windows-gnu"
  "aarch64-pc-windows-msvc"  # Requires Windows runner
  "x86_64-apple-darwin"       # Requires macOS
  "aarch64-apple-darwin"      # Requires macOS
)

for target in "${targets[@]}"; do
  cross build --release --target "$target"
done
```

### 2. Create Archives

```bash
VERSION="v0.3.12"

# Linux/macOS (tar.gz)
for target in x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin; do
  ARCHIVE="awful_aj-${VERSION}-${target}"
  mkdir -p "${ARCHIVE}"
  cp "target/${target}/release/aj" "${ARCHIVE}/"
  cp README.md LICENSE "${ARCHIVE}/"
  cp -r templates "${ARCHIVE}/"
  tar -czf "${ARCHIVE}.tar.gz" "${ARCHIVE}"
  shasum -a 256 "${ARCHIVE}.tar.gz" > "${ARCHIVE}.tar.gz.sha256"
done

# Windows (zip)
for target in x86_64-pc-windows-gnu aarch64-pc-windows-msvc; do
  ARCHIVE="awful_aj-${VERSION}-${target}"
  mkdir -p "${ARCHIVE}"
  cp "target/${target}/release/aj.exe" "${ARCHIVE}/"
  cp README.md LICENSE "${ARCHIVE}/"
  cp -r templates "${ARCHIVE}/"
  zip -r "${ARCHIVE}.zip" "${ARCHIVE}"
  shasum -a 256 "${ARCHIVE}.zip" > "${ARCHIVE}.zip.sha256"
done
```

### 3. Create GitHub Release

```bash
# Using GitHub CLI (gh)
gh release create "${VERSION}" \
  --title "Release ${VERSION}" \
  --notes "Release notes here" \
  awful_aj-${VERSION}-*.tar.gz \
  awful_aj-${VERSION}-*.tar.gz.sha256 \
  awful_aj-${VERSION}-*.zip \
  awful_aj-${VERSION}-*.zip.sha256
```

---

## Release Checklist

Before releasing:
- [ ] All tests pass (`cargo test`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy`)
- [ ] CHANGELOG updated (if you maintain one)
- [ ] Working tree is clean
- [ ] On main branch

After releasing:
- [ ] GitHub Actions workflow completes successfully
- [ ] All 6 binaries uploaded to GitHub Releases
- [ ] All 6 checksums uploaded
- [ ] Package appears on crates.io
- [ ] Test download and run binary
- [ ] Update README badges (if needed)

---

## Rollback

If you need to rollback a release:

```bash
# Use the Justfile rollback command
just rollback

# This will:
# 1. Delete the latest tag locally and remotely
# 2. Revert Cargo.toml version
# 3. Reset HEAD to previous commit
# 4. Require force push: git push origin main --force
```

**Warning**: Rollback requires force-push. Use with caution on shared branches.

---

## CI/CD Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                   Developer Workflow                        │
│                                                             │
│  1. just release patch                                      │
│     ├─ Bump version in Cargo.toml                          │
│     ├─ Commit + Tag (v0.3.12)                              │
│     ├─ Publish to crates.io                                │
│     └─ Push to GitHub (main + tag)                         │
└───────────────────┬─────────────────────────────────────────┘
                    │
                    ▼
┌─────────────────────────────────────────────────────────────┐
│              GitHub Actions Workflow                        │
│                                                             │
│  Trigger: on.push.tags (v*)                                 │
│                                                             │
│  Job 1: create-release                                      │
│    └─ Create GitHub Release draft                          │
│                                                             │
│  Job 2: build (matrix: 6 targets)                          │
│    ├─ x86_64-unknown-linux-gnu (Ubuntu + cross)            │
│    ├─ aarch64-unknown-linux-gnu (Ubuntu + cross)           │
│    ├─ x86_64-pc-windows-gnu (Ubuntu + cross)               │
│    ├─ aarch64-pc-windows-msvc (Windows native)             │
│    ├─ x86_64-apple-darwin (macOS native)                   │
│    └─ aarch64-apple-darwin (macOS native)                  │
│                                                             │
│  Each build:                                                │
│    1. Checkout code                                         │
│    2. Setup Rust + target                                   │
│    3. Build binary (cross or native)                        │
│    4. Sign binary (if secrets configured)                   │
│    5. Create archive (tar.gz or zip)                        │
│    6. Generate SHA256 checksum                              │
│    7. Upload to GitHub Release                              │
└─────────────────────────────────────────────────────────────┘
```

---

## Questions?

- **Workflow issues**: Check GitHub Actions logs
- **Signing setup**: See [SIGNING.md](SIGNING.md)
- **Build errors**: Check [Cross.toml](Cross.toml) and [rust-toolchain.toml](rust-toolchain.toml)
- **Justfile issues**: See [Justfile](Justfile)

For more info: https://github.com/graves/awful_aj
