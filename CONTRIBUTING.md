# Contributing to ClipCash NFT Smart Contract

Thank you for your interest in contributing to the ClipCash NFT smart contract! This document will help you set up your development environment and submit pull requests.

## Prerequisites

Before you begin, ensure you have the following installed:

- **Rust** (1.70 or later) - [Install Rust](https://rustup.rs/)
- **wasm32-unknown-unknown target** - Run: `rustup target add wasm32-unknown-unknown`
- **stellar-cli** (optional, for deployment) - Run: `cargo install stellar-cli`

## Development Setup

### 1. Clone the Repository

```bash
git clone https://github.com/your-repo/clips-contract.git
cd clips-contract
```

### 2. Install Dependencies

```bash
cd contracts
make install-deps
```

Or manually:

```bash
rustup target add wasm32-unknown-unknown
```

### 3. Verify Setup

Build the contract to ensure everything works:

```bash
make build
```

Run tests:

```bash
make test
```

## Making Changes

### Code Style

- Use `cargo fmt` to format your code before committing
- Follow the existing code patterns
- Add comments for complex logic

### Testing

Always test your changes:

```bash
# Run all tests
make test

# Run with verbose output
make test-verbose
```

### Building

Build the WASM contract:

```bash
make build
```

The output will be at: `contracts/target/wasm32-unknown-unknown/release/clips_nft.wasm`

## Submitting a Pull Request

### Step 1: Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/bug-description
```

### Step 2: Make Your Changes

1. Make your code changes
2. Add or update tests
3. Run `make test` to ensure everything passes
4. Run `make format` to format code
5. Run `make check` to verify no compilation errors

### Step 3: Commit Your Changes

Write clear commit messages:

```bash
git add .
git commit -m "feat: add royalty calculation for NFT transfers"
```

### Step 4: Push and Create PR

```bash
git push origin feature/your-feature-name
```

Then create a pull request through GitHub.

## Common Commands

| Command | Description |
|---------|-------------|
| `make build` | Build WASM contract |
| `make test` | Run tests |
| `make check` | Check code |
| `make format` | Format code |
| `make clean` | Clean build artifacts |

## Project Structure

```
contracts/
├── clips_nft/          # Smart contract source
│   ├── src/
│   │   └── lib.rs     # Main contract code
│   └── Cargo.toml     # Dependencies
├── Makefile           # Build commands
└── CONTRIBUTING.md     # This file
```

## Getting Help

- Open an issue for bugs or feature requests
- Check existing issues and PRs before creating new ones

## License

By contributing, you agree that your contributions will be licensed under the project's license.
