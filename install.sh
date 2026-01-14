#!/bin/sh
# ttl installer - https://github.com/lance0/ttl
# Usage: curl -fsSL https://raw.githubusercontent.com/lance0/ttl/master/install.sh | sh

set -e

REPO="lance0/ttl"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)

case "$OS" in
  linux)
    case "$ARCH" in
      x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
      aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
      arm64)   TARGET="aarch64-unknown-linux-gnu" ;;
      *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  darwin)
    case "$ARCH" in
      arm64)   TARGET="aarch64-apple-darwin" ;;
      aarch64) TARGET="aarch64-apple-darwin" ;;
      x86_64)  echo "Intel Mac not supported. Use: cargo install ttl"; exit 1 ;;
      *) echo "Unsupported architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"
    exit 1
    ;;
esac

# Get latest version
VERSION=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" | grep '"tag_name"' | cut -d'"' -f4)
if [ -z "$VERSION" ]; then
  echo "Failed to get latest version"
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$VERSION/ttl-$TARGET.tar.gz"

echo "Installing ttl $VERSION for $TARGET..."

# Download and extract
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

curl -fsSL "$URL" | tar xz -C "$TMPDIR"

# Install
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMPDIR/ttl" "$INSTALL_DIR/ttl"
else
  echo "Installing to $INSTALL_DIR (requires sudo)..."
  sudo mv "$TMPDIR/ttl" "$INSTALL_DIR/ttl"
fi

echo "Installed ttl to $INSTALL_DIR/ttl"
echo ""
echo "To run without sudo (Linux):"
echo "  sudo setcap cap_net_raw+ep $INSTALL_DIR/ttl"
echo ""
echo "Usage: ttl 8.8.8.8"
