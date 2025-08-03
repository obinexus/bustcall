#!/bin/bash
set -euo pipefail

echo "🔧 [OBINexus] Formatting Rust code..."
cargo fmt --all

echo "🔍 [OBINexus] Linting with clippy..."
cargo clippy --all-targets --all-features -- -D warnings

echo "📦 [OBINexus] Building core library..."
cargo build --release --lib

echo "🚀 [OBINexus] Building CLI binary..."
cargo build --release --features cli

echo "🧪 [OBINexus] Running test suite..."
cargo test --all-features

echo "🔗 [OBINexus] Testing FFI bindings..."
cargo test --features python-bindings

echo "✅ [OBINexus] All validation checks passed - ready for deployment"
