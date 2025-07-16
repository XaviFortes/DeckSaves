# GitHub Actions Workflows

This directory contains automated workflows for building, testing, and releasing DeckSaves.

## Workflows Overview

### 🏗️ Build Pipeline (`build.yml`)
**Triggers:** Push to main/develop, tags, PRs, manual dispatch

- **Test Suite**: Runs unit tests for Rust and frontend code
- **Multi-platform Builds**: Builds for macOS (ARM64/x64), Linux (x64), Windows (x64)
- **Artifacts**: Generates installers, packages, and CLI binaries
- **Release**: Automatically creates GitHub releases for version tags

**Platforms:**
- macOS: `.dmg` and `.app` bundles
- Linux: `.deb` packages and `.AppImage`
- Windows: `.msi` and `.exe` installers
- CLI: Standalone binaries for all platforms

### 🚀 Development Build (`dev-build.yml`)
**Triggers:** Feature branches, develop branch, PRs

- **Quick Tests**: Fast linting and formatting checks
- **Development Build**: Linux-only build for quick validation
- **Artifacts**: Short-term development artifacts (7 days retention)

### 🔍 Code Quality (`quality.yml`)
**Triggers:** Push to main/develop, PRs, weekly schedule

- **Rust Quality**: `cargo fmt`, `cargo clippy`, dependency checks
- **Frontend Quality**: Prettier, ESLint, TypeScript checks
- **Security Audit**: `cargo audit`, `npm audit`
- **Code Coverage**: Test coverage reporting with Codecov
- **Dependency Review**: Automated dependency security review

### 📦 Release (`release.yml`)
**Triggers:** Published releases, manual dispatch

- **Version Management**: Automatically updates version numbers
- **Multi-platform Release**: Comprehensive release builds
- **Code Signing**: Supports Tauri app signing (requires secrets)
- **Checksums**: Generates SHA256 checksums for all binaries
- **Crates.io**: Publishes Rust crates
- **Docker**: Builds and pushes Docker images

### 🔄 Dependencies (`dependencies.yml`)
**Triggers:** Weekly schedule (Mondays 9 AM UTC), manual dispatch

- **Rust Updates**: Updates `Cargo.lock` with compatible versions
- **NPM Updates**: Updates `package-lock.json` with compatible versions
- **Security Monitoring**: Creates issues for security vulnerabilities
- **Automated PRs**: Creates pull requests for dependency updates

## Setup Requirements

### Required Secrets

For full functionality, add these secrets to your GitHub repository:

```bash
# Tauri Code Signing (Optional)
TAURI_PRIVATE_KEY=<base64-encoded-private-key>
TAURI_KEY_PASSWORD=<private-key-password>

# Crates.io Publishing (Optional)
CARGO_REGISTRY_TOKEN=<crates.io-api-token>

# Docker Hub (Optional)
DOCKERHUB_USERNAME=<docker-hub-username>
DOCKERHUB_TOKEN=<docker-hub-access-token>

# GitHub Token (Automatically provided)
GITHUB_TOKEN=<automatically-provided>
```

### Repository Settings

1. **Enable Actions**: Go to Settings → Actions → General
2. **Workflow Permissions**: Set to "Read and write permissions"
3. **Branch Protection**: Configure branch protection rules for `main`

## Usage

### Triggering Builds

- **Automatic**: Push to main/develop or create PRs
- **Manual**: Go to Actions tab → Select workflow → "Run workflow"
- **Release**: Create and publish a release with a version tag (e.g., `v1.0.0`)

### Version Tagging

```bash
# Create and push a version tag
git tag v1.0.0
git push origin v1.0.0

# This will trigger the full release pipeline
```

### Development Workflow

1. **Feature Development**: Work on `feature/*` branches
2. **Quality Checks**: Push triggers quality checks automatically
3. **Pull Request**: Create PR to `develop` or `main`
4. **Review**: Automated checks must pass before merge
5. **Release**: Tag and release from `main` branch

## Build Artifacts

### Desktop Applications
- **macOS**: Universal app bundle and DMG installer
- **Linux**: DEB package and portable AppImage
- **Windows**: MSI installer and NSIS executable

### CLI Tool
- Standalone binaries for all platforms
- Cross-compiled from GitHub Actions runners
- SHA256 checksums for verification

### Docker Images
- Multi-architecture images (AMD64, ARM64)
- Published to Docker Hub
- Tagged with version numbers

## Monitoring

### Build Status
- Check the Actions tab for build status
- Failing builds will block merges (if branch protection is enabled)
- Email notifications for failed builds on main branches

### Security
- Weekly security audits
- Automatic issues created for vulnerabilities
- Dependency update PRs created weekly

### Quality Metrics
- Code coverage reports via Codecov
- Rust and frontend code quality checks
- Outdated dependency notifications

## Troubleshooting

### Common Issues

1. **Build Failures**: Check the specific job logs in Actions tab
2. **Missing Dependencies**: Ensure all required tools are installed in workflows
3. **Permission Errors**: Check repository permissions and secrets
4. **Code Signing**: Verify Tauri signing certificates are valid

### Local Testing

```bash
# Test Rust builds locally
cargo test --workspace
cargo clippy --all-targets --all-features
cargo fmt --check

# Test frontend builds locally
npm ci
npm run build
npm run lint
npm run type-check

# Test Tauri build locally
cd tauri-ui
cargo tauri build
```

## Customization

To modify the workflows:

1. **Build Targets**: Edit the matrix in `build.yml`
2. **Quality Rules**: Modify linting rules in `quality.yml`
3. **Release Process**: Customize `release.yml` for your needs
4. **Dependencies**: Adjust update frequency in `dependencies.yml`

Remember to test workflow changes in a fork before applying to the main repository.
