codecov.yml
.github/workflows/coverage.yml

# Code Coverage Setup

This project uses [Codecov](https://codecov.io/) for code coverage reporting.

## Initial Setup

1. Sign in to [codecov.io](https://codecov.io/) with your GitHub account
2. Add your `openmm` repository
3. Copy the repository upload token
4. Add it as a repository secret:
   - Go to your GitHub repository → Settings → Secrets and variables → Actions
   - Click "New repository secret"
   - Name: `CODECOV_TOKEN`
   - Value: paste your Codecov upload token
   - Click "Add secret"

## Viewing Coverage

Once set up, coverage reports will be:
- Generated automatically on every push and PR
- Visible in PR comments
- Available at `https://codecov.io/gh/roarc0/openmm`
- Displayed in the README badge

## Local Coverage

To generate coverage locally:

```bash
# Install cargo-tarpaulin (Linux only)
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --verbose --all-features --workspace --timeout 300

# Generate HTML report
cargo tarpaulin --verbose --all-features --workspace --timeout 300 --out html
```

Note: `cargo-tarpaulin` only works on Linux. For other platforms, consider using:
- [cargo-llvm-cov](https://github.com/taiki-e/cargo-llvm-cov) (cross-platform)
- Docker to run tarpaulin in a Linux container
