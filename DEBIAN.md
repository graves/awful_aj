# Debian Package Guide for awful_aj

This guide covers building, installing, and distributing `.deb` packages for awful_aj.

## Overview

The project automatically builds Debian packages for:
- **x86_64 (amd64)**: Standard Intel/AMD 64-bit systems
- **ARM64 (aarch64)**: ARM-based systems (Raspberry Pi 4+, AWS Graviton, etc.)

Debian packages are built using [`cargo-deb`](https://github.com/kornelski/cargo-deb), which creates standard `.deb` files from Cargo metadata.

---

## Installation from GitHub Releases

### Quick Install (One-liner)

**x86_64 (most Linux desktops/servers):**
```bash
curl -LO https://github.com/graves/awful_aj/releases/latest/download/awful-aj_latest_amd64.deb
sudo dpkg -i awful-aj_latest_amd64.deb
```

**ARM64 (Raspberry Pi, ARM servers):**
```bash
curl -LO https://github.com/graves/awful_aj/releases/latest/download/awful-aj_latest_arm64.deb
sudo dpkg -i awful-aj_latest_arm64.deb
```

### Manual Download and Install

1. **Download the package** from [GitHub Releases](https://github.com/graves/awful_aj/releases):
   ```bash
   # Replace VERSION with the actual version (e.g., v0.3.11)
   VERSION="v0.3.11"
   ARCH="amd64"  # or "arm64"

   curl -LO "https://github.com/graves/awful_aj/releases/download/${VERSION}/awful-aj_${VERSION}_${ARCH}.deb"
   ```

2. **Verify checksum** (optional but recommended):
   ```bash
   curl -LO "https://github.com/graves/awful_aj/releases/download/${VERSION}/awful-aj_${VERSION}_${ARCH}.deb.sha256"
   sha256sum -c "awful-aj_${VERSION}_${ARCH}.deb.sha256"
   ```

3. **Install the package**:
   ```bash
   sudo dpkg -i "awful-aj_${VERSION}_${ARCH}.deb"
   ```

4. **Fix dependencies** (if needed):
   ```bash
   sudo apt-get install -f
   ```

---

## Building Debian Packages Locally

### Prerequisites

1. **Install cargo-deb**:
   ```bash
   cargo install cargo-deb
   ```

2. **Ensure dependencies** for cross-compilation (if building for different architectures):
   ```bash
   cargo install cross --git https://github.com/cross-rs/cross
   ```

### Build for Native Architecture

```bash
# Build the binary first
cargo build --release

# Create the .deb package
cargo deb

# Output: target/debian/awful-aj_0.3.11_amd64.deb (or _arm64.deb)
```

### Build for x86_64 (amd64)

```bash
# Build binary with cross
cross build --release --target x86_64-unknown-linux-gnu

# Copy to expected location
mkdir -p target/release
cp target/x86_64-unknown-linux-gnu/release/aj target/release/aj

# Build .deb
cargo deb --target x86_64-unknown-linux-gnu --no-build

# Output: target/x86_64-unknown-linux-gnu/debian/awful-aj_0.3.11_amd64.deb
```

### Build for ARM64 (aarch64)

```bash
# Build binary with cross
cross build --release --target aarch64-unknown-linux-gnu

# Copy to expected location
mkdir -p target/release
cp target/aarch64-unknown-linux-gnu/release/aj target/release/aj

# Build .deb
cargo deb --target aarch64-unknown-linux-gnu --no-build

# Output: target/aarch64-unknown-linux-gnu/debian/awful-aj_0.3.11_arm64.deb
```

### Build Script (All Architectures)

Create `build-deb.sh`:
```bash
#!/bin/bash
set -e

VERSION=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')

echo "Building Debian packages for awful_aj v${VERSION}..."

# Install cargo-deb if not present
if ! command -v cargo-deb &> /dev/null; then
    echo "Installing cargo-deb..."
    cargo install cargo-deb
fi

# x86_64 (amd64)
echo "Building for x86_64..."
cross build --release --target x86_64-unknown-linux-gnu
mkdir -p target/release
cp target/x86_64-unknown-linux-gnu/release/aj target/release/aj
cargo deb --target x86_64-unknown-linux-gnu --no-build

# ARM64 (aarch64)
echo "Building for ARM64..."
cross build --release --target aarch64-unknown-linux-gnu
cp target/aarch64-unknown-linux-gnu/release/aj target/release/aj
cargo deb --target aarch64-unknown-linux-gnu --no-build

echo "✓ Build complete!"
echo ""
echo "Packages:"
find target -name "*.deb" -type f
```

Run it:
```bash
chmod +x build-deb.sh
./build-deb.sh
```

---

## Package Contents

After installation, files are placed at:

| File | Location | Description |
|------|----------|-------------|
| `aj` | `/usr/bin/aj` | Main executable |
| `README.md` | `/usr/share/doc/awful-aj/README.md` | Documentation |
| `LICENSE` | `/usr/share/doc/awful-aj/LICENSE` | License file |
| Templates | `/usr/share/awful-aj/templates/*.yaml` | YAML templates |

### Configuration

User configuration is stored in:
- **Linux**: `~/.config/aj/config.yaml`

The package **does not** create default configuration. Users must run:
```bash
aj init
```

---

## Package Metadata

The `.deb` package includes:

| Field | Value |
|-------|-------|
| Package Name | `awful-aj` |
| Maintainer | Thomas Gentry <thomas@awfulsec.com> |
| Section | `utils` |
| Priority | `optional` |
| Architecture | `amd64` or `arm64` |
| Depends | Auto-detected system libraries |
| Description | CLI for interacting with OpenAI-compatible APIs |

To view metadata:
```bash
dpkg-deb -I awful-aj_0.3.11_amd64.deb
```

---

## Upgrading

### From .deb Package

```bash
# Download new version
curl -LO "https://github.com/graves/awful_aj/releases/download/v0.4.0/awful-aj_v0.4.0_amd64.deb"

# Install (upgrades automatically)
sudo dpkg -i awful-aj_v0.4.0_amd64.deb
```

### From crates.io (if previously installed via cargo)

```bash
# Uninstall cargo version first
cargo uninstall awful_aj

# Then install .deb
sudo dpkg -i awful-aj_v0.3.11_amd64.deb
```

---

## Uninstalling

```bash
# Remove package
sudo dpkg -r awful-aj

# Remove package and configuration
sudo dpkg -P awful-aj
```

**Note**: User configuration in `~/.config/aj/` is **not** removed by `dpkg -r`. Use `dpkg -P` to purge configuration, or manually delete:
```bash
rm -rf ~/.config/aj/
```

---

## Troubleshooting

### "Package has unmet dependencies"

```bash
# Install missing dependencies
sudo apt-get install -f
```

### "dpkg: error processing package"

Check system requirements:
- **OS**: Debian 10+, Ubuntu 20.04+, or compatible
- **Architecture**: x86_64 (amd64) or ARM64 (aarch64)
- **Disk space**: ~100MB free

### "command not found: aj"

Ensure `/usr/bin` is in your `$PATH`:
```bash
echo $PATH
# Should include /usr/bin

# If not, add to ~/.bashrc or ~/.zshrc:
export PATH="/usr/bin:$PATH"
```

### Verify Installation

```bash
# Check package is installed
dpkg -l | grep awful-aj

# Check binary exists
which aj

# Test binary
aj --version
```

---

## Creating a Custom Repository (Advanced)

For organizations that want to host their own `.deb` repository:

### 1. Set Up Repository Structure

```bash
mkdir -p debian-repo/pool/main/a/awful-aj
cp awful-aj_*.deb debian-repo/pool/main/a/awful-aj/
```

### 2. Generate Package Index

```bash
cd debian-repo
apt-ftparchive packages pool/ > Packages
gzip -k Packages
apt-ftparchive release . > Release
```

### 3. Sign Repository (Optional)

```bash
# Generate GPG key (if you don't have one)
gpg --gen-key

# Sign Release file
gpg --clearsign -o InRelease Release
```

### 4. Host Repository

Serve the `debian-repo/` directory via HTTP/HTTPS:
```bash
# Example with nginx
sudo cp -r debian-repo /var/www/html/debian

# Or with Python (testing only)
cd debian-repo
python3 -m http.server 8080
```

### 5. Client Configuration

On client machines:
```bash
# Add repository
echo "deb [trusted=yes] http://your-server/debian /" | sudo tee /etc/apt/sources.list.d/awful-aj.list

# Update and install
sudo apt update
sudo apt install awful-aj
```

---

## GitHub Release Integration

The `.deb` packages are automatically built and uploaded by GitHub Actions when you push a tag.

### Release Artifacts

Each release includes:
```
awful-aj_v0.3.11_amd64.deb          # x86_64 Debian package
awful-aj_v0.3.11_amd64.deb.sha256   # Checksum
awful-aj_v0.3.11_arm64.deb          # ARM64 Debian package
awful-aj_v0.3.11_arm64.deb.sha256   # Checksum
```

Plus the standard tar.gz and zip archives for other platforms.

---

## Comparison: .deb vs cargo install vs tar.gz

| Method | Pros | Cons |
|--------|------|------|
| **`.deb`** | System integration, dependency management, easy updates via `apt` | Linux-only, requires root |
| **`cargo install`** | Always latest version, no root needed | Slower (compiles from source), requires Rust toolchain |
| **`tar.gz`** | No root needed, portable | Manual PATH setup, no update mechanism |

**Recommendation**: Use `.deb` for production servers and workstations. Use `cargo install` for development.

---

## Testing the Package

### Automated Testing

```bash
# Install in a Docker container
docker run --rm -it debian:bookworm bash

# Inside container:
apt-get update
apt-get install -y curl
curl -LO https://github.com/graves/awful_aj/releases/latest/download/awful-aj_latest_amd64.deb
dpkg -i awful-aj_latest_amd64.deb
aj --version
```

### Manual Testing Checklist

- [ ] Package installs without errors
- [ ] `aj --version` shows correct version
- [ ] `aj init` creates configuration
- [ ] Templates are accessible in `/usr/share/awful-aj/templates/`
- [ ] `aj ask "test question"` works (with API configured)
- [ ] Package upgrades cleanly
- [ ] Package uninstalls cleanly

---

## References

- **cargo-deb**: https://github.com/kornelski/cargo-deb
- **Debian Policy**: https://www.debian.org/doc/debian-policy/
- **dpkg Manual**: https://man7.org/linux/man-pages/man1/dpkg.1.html
- **GitHub Releases**: https://github.com/graves/awful_aj/releases

---

## Support

For issues with Debian packages:
- Check existing issues: https://github.com/graves/awful_aj/issues
- Create new issue: https://github.com/graves/awful_aj/issues/new
- Email: thomas@awfulsec.com
