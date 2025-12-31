#!/bin/bash
set -e

echo "🔨 Building OpenAI Unofficial WASM Component..."

# Ensure WASM target is installed
if ! rustup target list --installed | grep -q "wasm32-wasip1"; then
    echo "📦 Installing wasm32-wasip1 target..."
    rustup target add wasm32-wasip1
fi

# Check if wasm-tools is available
if ! command -v wasm-tools &> /dev/null; then
    echo "❌ wasm-tools not found!"
    echo "   Install with: cargo install wasm-tools"
    exit 1
fi

# Build for WASM target
echo "🏗️  Compiling to WASM module..."
cargo build --target wasm32-wasip1 --release

# Use local target directory
WASM_MODULE="target/wasm32-wasip1/release/openai_unofficial_wasm.wasm"

# Convert WASM module to component
echo "🔄 Converting WASM module to component..."
WASM_COMPONENT="target/wasm32-wasip1/release/openai_unofficial_wasm_component.wasm"

# Check if we have a WASI adapter
ADAPTER_URL="https://github.com/bytecodealliance/wasmtime/releases/download/v25.0.3/wasi_snapshot_preview1.reactor.wasm"
ADAPTER_FILE="target/wasm32-wasip1/release/wasi_snapshot_preview1.reactor.wasm"

if [ ! -f "$ADAPTER_FILE" ]; then
    echo "📥 Downloading WASI adapter..."
    mkdir -p target/wasm32-wasip1/release
    curl -L -o "$ADAPTER_FILE" "$ADAPTER_URL"
fi

wasm-tools component new "$WASM_MODULE" --adapt "wasi_snapshot_preview1=$ADAPTER_FILE" -o "$WASM_COMPONENT"

# Check if wasm-opt is available for optimization
if command -v wasm-opt &> /dev/null; then
    echo "⚡ Optimizing WASM component..."
    wasm-opt -Oz -o target/wasm32-wasip1/release/openai_unofficial_wasm_opt.wasm "$WASM_COMPONENT"
    WASM_FILE="target/wasm32-wasip1/release/openai_unofficial_wasm_opt.wasm"
else
    echo "ℹ️  wasm-opt not found, skipping optimization"
    WASM_FILE="$WASM_COMPONENT"
fi

# Create output directory
mkdir -p wasm-output

# Copy WASM binary
echo "📦 Copying WASM component to wasm-output/"
cp "$WASM_FILE" wasm-output/openai_unofficial_wasm.wasm

# Show size
SIZE=$(du -h wasm-output/openai_unofficial_wasm.wasm | cut -f1)
echo "✅ WASM component built successfully!"
echo "   Location: wasm-output/openai_unofficial_wasm.wasm"
echo "   Size: $SIZE"

# Installation instructions
echo ""
echo "📋 To install, run:"
echo "   mkdir -p ~/.trustee/extensions/openai-unofficial"
echo "   cp wasm-output/openai_unofficial_wasm.wasm ~/.trustee/extensions/openai-unofficial/"
echo "   cp extension.toml ~/.trustee/extensions/openai-unofficial/"
