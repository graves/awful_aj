# Code Signing Setup for awful_aj

This document explains how to set up code signing for macOS and Windows builds in GitHub Actions.

## Overview

The GitHub Actions workflow ([.github/workflows/release.yml](.github/workflows/release.yml)) supports optional code signing for:
- **macOS**: Using Apple Developer certificates
- **Windows**: Using Authenticode certificates

Signing is **optional** - if secrets are not configured, the workflow will build unsigned binaries.

---

## macOS Code Signing

### Prerequisites

1. **Apple Developer Account** with a Developer ID Application certificate
2. Certificate exported as `.p12` file

### Setup Steps

#### 1. Export Certificate from Keychain

On your Mac:

```bash
# Open Keychain Access
# Find "Developer ID Application: Your Name"
# Right-click → Export "Developer ID Application: Your Name..."
# Save as certificate.p12 with a strong password
```

#### 2. Encode Certificate to Base64

```bash
base64 -i certificate.p12 -o certificate.txt
```

#### 3. Add GitHub Secrets

Go to your repository → Settings → Secrets and variables → Actions → New repository secret

Add these secrets:

| Secret Name | Value | Description |
|-------------|-------|-------------|
| `MACOS_CERTIFICATE` | Contents of `certificate.txt` | Base64-encoded .p12 file |
| `MACOS_CERTIFICATE_PWD` | Your .p12 password | Password for the certificate |
| `MACOS_KEYCHAIN_PWD` | Any strong password | Temporary keychain password |
| `MACOS_SIGNING_IDENTITY` | e.g., "Developer ID Application: Your Name (TEAM_ID)" | Signing identity name |

#### 4. Find Your Signing Identity

```bash
security find-identity -v -p codesigning
```

Look for something like:
```
1) ABC123DEF456 "Developer ID Application: Your Name (TEAM_ID)"
```

Use the full quoted string as `MACOS_SIGNING_IDENTITY`.

### Testing

After pushing a tag, check the workflow logs for:
```
Signing macOS binary...
codesign --verify --verbose target/...
✓ Signature verified
```

---

## Windows Code Signing

### Prerequisites

1. **Code Signing Certificate** (from DigiCert, Sectigo, etc.)
2. Certificate as `.pfx` file

### Setup Steps

#### 1. Export Certificate as PFX

If you have a certificate in Windows Certificate Store:

```powershell
# Open Certificate Manager (certmgr.msc)
# Find your code signing certificate
# Right-click → All Tasks → Export
# Choose "Yes, export the private key"
# Save as certificate.pfx with a password
```

#### 2. Encode Certificate to Base64

**Windows (PowerShell):**
```powershell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("certificate.pfx")) | Out-File certificate.txt
```

**macOS/Linux:**
```bash
base64 -i certificate.pfx -o certificate.txt
```

#### 3. Add GitHub Secrets

| Secret Name | Value | Description |
|-------------|-------|-------------|
| `WINDOWS_CERTIFICATE` | Contents of `certificate.txt` | Base64-encoded .pfx file |
| `WINDOWS_CERTIFICATE_PWD` | Your .pfx password | Password for the certificate |

### Testing

After pushing a tag, check the workflow logs for:
```
Signing Windows binary...
AzureSignTool sign -f certificate.pfx...
✓ Signature applied
```

---

## Verification

### Verify macOS Signature

```bash
# Download the release binary
tar -xzf awful_aj-v0.3.11-aarch64-apple-darwin.tar.gz
cd awful_aj-v0.3.11-aarch64-apple-darwin

# Check signature
codesign --verify --verbose aj
# Should output: "valid on disk" and "satisfies its Designated Requirement"

# View signature details
codesign -dv aj
```

### Verify Windows Signature

```powershell
# Download the release binary
Expand-Archive awful_aj-v0.3.11-x86_64-pc-windows-msvc.zip
cd awful_aj-v0.3.11-x86_64-pc-windows-msvc

# Check signature
Get-AuthenticodeSignature aj.exe
# Status should be "Valid"
```

---

## Troubleshooting

### macOS: "No identity found"

- Ensure `MACOS_SIGNING_IDENTITY` matches the output of `security find-identity`
- Double-check the certificate password
- Verify the certificate hasn't expired

### Windows: "AzureSignTool not found"

The workflow installs it automatically. If it fails:
- Check .NET is installed in the runner
- Verify the certificate is valid

### Unsigned Builds

If secrets are not configured, builds will proceed **without signing**. The workflow logs will show:
```
No macOS certificate configured, skipping signing
```
or
```
No Windows certificate configured, skipping signing
```

This is normal and expected if you don't need signed builds.

---

## Security Best Practices

1. **Never commit** `.p12` or `.pfx` files to the repository
2. **Use strong passwords** for certificate files
3. **Rotate secrets** annually or when team members leave
4. **Restrict secret access** to required workflows only
5. **Monitor certificate expiration** dates

---

## Cost Considerations

| Platform | Certificate Type | Annual Cost | Validity |
|----------|-----------------|-------------|----------|
| macOS | Developer ID | $99/year | 1 year |
| Windows | Code Signing (OV) | $150-400/year | 1-3 years |
| Windows | EV Code Signing | $300-600/year | 1-3 years |

**Note**: EV (Extended Validation) certificates provide instant SmartScreen reputation on Windows.

---

## Alternative: Local Signing

If you prefer to sign locally before release:

### macOS
```bash
# Build for macOS targets
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

# Sign locally
codesign --force --sign "Developer ID Application: Your Name" \
  --options runtime --timestamp \
  target/aarch64-apple-darwin/release/aj

codesign --force --sign "Developer ID Application: Your Name" \
  --options runtime --timestamp \
  target/x86_64-apple-darwin/release/aj
```

### Windows
```powershell
# Build for Windows
cargo build --release --target x86_64-pc-windows-msvc

# Sign with signtool (from Windows SDK)
signtool sign /f certificate.pfx /p "password" /tr http://timestamp.digicert.com /td sha256 /fd sha256 target\x86_64-pc-windows-msvc\release\aj.exe
```

Then manually upload signed binaries to GitHub Releases.

---

## References

- [Apple Code Signing Guide](https://developer.apple.com/support/code-signing/)
- [Windows Authenticode](https://docs.microsoft.com/windows/win32/seccrypto/cryptography-tools)
- [GitHub Actions Encrypted Secrets](https://docs.github.com/actions/security-guides/encrypted-secrets)
