# Git Flow Guide for awful_aj

This document explains the Git Flow branching strategy and release process for this project.

## Branch Structure

```
main (production) ← GitHub Release triggers here
 ↑
dev (integration) ← Default branch for development
 ↑
feature/* (work in progress)
```

### Branch Purposes

| Branch | Purpose | Protected | Auto-Release |
|--------|---------|-----------|--------------|
| `main` | Production-ready code | ✅ Yes | ✅ Yes (on merge) |
| `dev` | Integration branch | ⚠️  Optional | ❌ No |
| `feature/*` | Individual features/fixes | ❌ No | ❌ No |

---

## Initial Setup (One-Time)

### 1. Create `dev` Branch Locally

```bash
# If you're currently on main with uncommitted changes
git checkout -b dev

# Push dev to remote
git push -u origin dev
```

### 2. Configure GitHub Repository

Go to **https://github.com/graves/awful_aj/settings/branches**:

#### Set Default Branch
1. Change default branch from `main` to `dev`
2. This makes new clones and PRs default to `dev`

#### Protect `main` Branch
Add these rules for `main`:
- ✅ Require a pull request before merging
- ✅ Require approvals: **1** (or 0 if solo)
- ✅ Require status checks to pass
- ✅ Require branches to be up to date
- ✅ Do not allow bypassing (recommended)

#### Protect `dev` Branch (Optional but Recommended)
Add these rules for `dev`:
- ✅ Require a pull request before merging
- ✅ Require status checks to pass

### 3. Verify GitHub Actions Permissions

Go to **Settings → Actions → General → Workflow permissions**:
- Select "Read and write permissions"
- ✅ Allow GitHub Actions to create and approve pull requests

---

## Workflow: Feature Development

### 1. Start a New Feature

```bash
# Create feature branch from dev
just feature my-cool-feature

# This runs:
# - git checkout dev
# - git pull origin dev
# - git checkout -b feature/my-cool-feature
```

### 2. Develop Your Feature

```bash
# Make changes
vim src/main.rs

# Commit frequently
git add .
git commit -m "Add cool feature"

# Continue working...
git commit -m "Refine cool feature"
git commit -m "Add tests for cool feature"
```

### 3. Push and Create PR to `dev`

```bash
# Push feature branch and get PR URL
just finish-feature

# This outputs:
# ✓ Branch pushed!
# 📝 Create PR: https://github.com/graves/awful_aj/compare/dev...feature/my-cool-feature?expand=1
```

Click the URL to create a PR:
- **Base**: `dev`
- **Compare**: `feature/my-cool-feature`
- Add description, request review (if team), merge

### 4. Clean Up (After Merge)

```bash
# Switch back to dev and update
git checkout dev
git pull origin dev

# Delete local feature branch
git branch -d feature/my-cool-feature

# Delete remote feature branch (optional, GitHub can auto-delete)
git push origin --delete feature/my-cool-feature
```

---

## Workflow: Creating a Release

### 1. Prepare Release on `dev`

```bash
# Ensure you're on dev and it's clean
git checkout dev
git pull origin dev

# Bump version (choose one)
just prep-patch   # 0.3.15 → 0.3.16
just prep-minor   # 0.3.15 → 0.4.0
just prep-major   # 0.3.15 → 1.0.0

# This:
# - Bumps version in Cargo.toml
# - Commits: "Bump version to X.Y.Z"
# - Runs preflight checks
# - Tells you next steps
```

Output:
```
✓ Version bumped to 0.3.16 on dev
📋 Next steps:
  1. git push origin dev
  2. Create PR: dev → main
  3. Merge PR (triggers GitHub Actions release)
```

### 2. Push Version Bump to `dev`

```bash
git push origin dev
```

### 3. Create PR: `dev` → `main`

Go to: **https://github.com/graves/awful_aj/compare/main...dev**

- **Title**: `Release v0.3.16`
- **Description**:
  ```markdown
  ## Changes in this release
  - Feature: Add cool feature (#123)
  - Fix: Resolve bug in vector store (#124)
  - Docs: Update README with new examples

  ## Checklist
  - [x] Version bumped in Cargo.toml
  - [x] All tests passing
  - [x] CHANGELOG updated (if you maintain one)
  ```

### 4. Merge PR (Triggers Auto-Release)

When you merge the PR to `main`, GitHub Actions automatically:
1. ✅ Reads version from Cargo.toml (e.g., `0.3.16`)
2. ✅ Creates git tag `v0.3.16`
3. ✅ Builds binaries for all 6 platforms
4. ✅ Creates Debian packages (.deb)
5. ✅ Generates SHA256 checksums
6. ✅ Creates GitHub Release with all artifacts
7. ✅ Pushes tag to repository

**Monitor progress**: https://github.com/graves/awful_aj/actions

### 5. Publish to crates.io

After GitHub Actions completes successfully:

```bash
# Switch to main and pull the merge
git checkout main
git pull origin main

# Publish to crates.io
just publish-crates

# This:
# - Verifies you're on main
# - Runs cargo publish
```

### 6. Sync `dev` with `main`

```bash
# Update dev with the merge commit from main
git checkout dev
git merge main
git push origin dev
```

---

## Complete Example Workflow

```bash
# ── Feature Development ──
just feature add-json-export     # Create feature/add-json-export
# ... make changes, commit ...
just finish-feature              # Push and get PR URL
# Create PR, merge to dev

# ── Another Feature ──
just sync-dev                    # Update dev
just feature fix-memory-leak
# ... make changes, commit ...
just finish-feature
# Create PR, merge to dev

# ── Ready to Release ──
git checkout dev
git pull origin dev
just prep-minor                  # Bump to 0.4.0

git push origin dev
# Create PR: dev → main with title "Release v0.4.0"
# Merge PR → triggers GitHub Actions

# Wait for GitHub Actions to complete
# Then publish to crates.io:
git checkout main
git pull origin main
just publish-crates

# Sync dev
git checkout dev
git merge main
git push origin dev
```

---

## Justfile Commands Reference

### Git Flow Commands (New)

| Command | Description | Branch Requirement |
|---------|-------------|-------------------|
| `just feature <name>` | Create feature branch from dev | Any |
| `just finish-feature` | Push feature and show PR URL | feature/* |
| `just sync-dev` | Pull latest dev | Any |
| `just prep-patch` | Bump patch version on dev | dev |
| `just prep-minor` | Bump minor version on dev | dev |
| `just prep-major` | Bump major version on dev | dev |
| `just publish-crates` | Publish to crates.io | main |

### Legacy Commands (Deprecated)

| Command | Status | Alternative |
|---------|--------|-------------|
| `just release patch` | ⚠️  Deprecated | Use Git Flow (prep-patch + PR) |
| `just patch` | ⚠️  Deprecated | Use `just prep-patch` |
| `just minor` | ⚠️  Deprecated | Use `just prep-minor` |
| `just major` | ⚠️  Deprecated | Use `just prep-major` |

---

## GitHub Actions Behavior

### Old Behavior (Tag-Based)
```
git push origin v0.3.16  →  GitHub Actions  →  Build + Release
```

### New Behavior (Branch-Based)
```
Merge PR to main  →  GitHub Actions  →  Auto-tag + Build + Release
```

**Key difference**: You no longer manually create tags. GitHub Actions creates them automatically based on Cargo.toml version when code is merged to `main`.

---

## Troubleshooting

### "Tag already exists" Error

If GitHub Actions fails because the tag already exists:

1. **Check if a release was already created**:
   - Go to https://github.com/graves/awful_aj/releases
   - If release exists, you're done!

2. **If no release exists** (workflow failed mid-way):
   ```bash
   # Delete the orphaned tag
   git push origin --delete v0.3.16

   # Re-trigger workflow by pushing empty commit to main
   git checkout main
   git commit --allow-empty -m "Re-trigger release workflow"
   git push origin main
   ```

### Accidentally Merged Wrong Version

```bash
# Use rollback
just rollback

# Follow prompts to revert to previous version
# Then force-push to dev
git push origin dev --force
```

### Need to Hotfix Production

```bash
# Create hotfix branch from main
git checkout main
git pull origin main
git checkout -b hotfix/critical-bug

# Fix the bug
vim src/main.rs
git commit -am "Fix critical bug"

# Push and create PR to main (skip dev)
git push -u origin hotfix/critical-bug
# Create PR: hotfix/critical-bug → main

# After merge and release, backport to dev
git checkout dev
git merge main
git push origin dev
```

---

## Migration Checklist

To fully migrate to the new Git Flow:

- [ ] Create `dev` branch locally and push to GitHub
- [ ] Set `dev` as default branch in GitHub settings
- [ ] Add branch protection rules for `main`
- [ ] (Optional) Add branch protection rules for `dev`
- [ ] Verify GitHub Actions has write permissions
- [ ] Test the workflow with a dummy feature branch
- [ ] Update team documentation (if applicable)
- [ ] Stop using `just release` / `just patch` commands

---

## Benefits of This Workflow

✅ **Safer releases**: All code reviewed before reaching main
✅ **Automated releases**: No manual tagging or release creation
✅ **Clean history**: main only has merge commits (releases)
✅ **Parallel development**: Multiple features can be worked on simultaneously
✅ **Easy rollbacks**: Clear separation between dev and production
✅ **CI/CD friendly**: Every merge to main is a release

---

## Questions?

- **Why not use GitHub Flow (feature → main directly)?**
  - We want an integration branch (`dev`) to test multiple features together before release.

- **Why not use Git Flow (with release/ and hotfix/ branches)?**
  - Simpler is better. We don't need separate release branches since GitHub Actions automates releases.

- **Can I still manually create releases?**
  - Yes, but not recommended. Use `just release` with the deprecation warning, or manually push tags.

- **What if I need to test a release before publishing?**
  - Use draft releases: modify `.github/workflows/release.yml` to set `draft: true`, then manually publish from GitHub UI.

---

For more details on the release artifacts and distribution, see [RELEASE.md](RELEASE.md).
