# Contributing to YamlDB

Thank you for your interest in contributing to YamlDB!

## Getting Started

1. Fork the repository
2. Clone your fork
3. Create a feature branch
4. Make your changes
5. Run tests
6. Submit a pull request

## Development Setup

Rust tests require `yq` v4 on `PATH`, or a valid `YAMLDB_YQ` path. JDBC development additionally requires JDK 17 or newer.

```bash
# Clone the repo
git clone https://github.com/your-username/yamldb.git
cd yamldb

# Build
cargo build

# Run all Rust tests and examples
cargo test --all-targets

# Run clippy
cargo clippy --all-targets -- -D warnings

# Check formatting
cargo fmt --all -- --check

# Build and test JDBC (Linux/macOS)
bash jdbc/build.sh
```

On Windows, build and test JDBC with:

```powershell
powershell -ExecutionPolicy Bypass -File jdbc\build.ps1
```

## Code Style

- Follow Rust standard conventions
- Use `cargo fmt` to format code
- Use `cargo clippy` to check for warnings
- Add tests for new features
- Update documentation for API changes

## Commit Messages

Use conventional commits:

- `feat:` - New feature
- `fix:` - Bug fix
- `docs:` - Documentation
- `test:` - Tests
- `refactor:` - Code refactoring
- `chore:` - Maintenance

## Pull Requests

1. Keep PRs focused on a single change
2. Include tests for new functionality
3. Update documentation as needed
4. Ensure all tests pass
5. Ensure clippy has no warnings

## Reporting Issues

- Use GitHub Issues
- Include reproduction steps
- Include Rust version and OS
- Include error messages

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
