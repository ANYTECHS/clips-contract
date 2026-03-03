# Makefile for ClipCash NFT Smart Contract
# 
# This file provides convenient commands for building and testing
# the Stellar Soroban smart contract.

.PHONY: build test clean check install-deps

# Default target
all: build

# Build the WASM contract
build:
	@echo "Building ClipCash NFT WASM contract..."
	@cargo build --target wasm32-unknown-unknown --release -p clips_nft
	@echo "WASM contract built successfully!"

# Build for testing (faster, with debug info)
build-debug:
	@cargo build --target wasm32-unknown-unknown -p clips_nft

# Run tests
test:
	@cargo test -p clips_nft

# Run tests with output
test-verbose:
	@cargo test -p clips_nft -- --nocapture

# Check code without building
check:
	@cargo check -p clips_nft

# Format code
format:
	@cargo fmt -p clips_nft

# Lint code
lint:
	@cargo clippy -p clips_nft -- -D warnings

# Clean build artifacts
clean:
	@cargo clean -p clips_nft

# Install dependencies (Rust, wasm32 target)
install-deps:
	@echo "Installing Rust dependencies..."
	@rustup target add wasm32-unknown-unknown
	@echo "Dependencies installed!"

# Build and optimize WASM
optimize:
	@echo "Building optimized WASM..."
	@cargo build --target wasm32-unknown-unknown --release -p clips_nft
	@echo "Optimizing with wasm-opt..."
	@wasm-opt -Oz contracts/target/wasm32-unknown-unknown/release/clips_nft.wasm -o contracts/target/wasm32-unknown-unknown/release/clips_nft_optimized.wasm

# Watch mode for development
watch:
	@cargo watch -x check -x test -p clips_nft

# Show help
help:
	@echo "ClipCash NFT Smart Contract - Available Commands"
	@echo ""
	@echo "  make build         Build the WASM contract (release)"
	@echo "  make build-debug   Build with debug info"
	@echo "  make test          Run contract tests"
	@echo "  make test-verbose  Run tests with output"
	@echo "  make check         Check code without building"
	@echo "  make format        Format code"
	@echo "  make lint          Lint code"
	@echo "  make clean         Clean build artifacts"
	@echo "  make install-deps  Install Rust dependencies"
	@echo "  make optimize      Build and optimize WASM"
