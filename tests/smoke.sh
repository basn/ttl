#!/bin/bash
# Cross-platform smoke tests for ttl
# Run as: sudo ./tests/smoke.sh
#
# Environment variables:
#   TTL    - Path to ttl binary (default: ./target/release/ttl)
#   TARGET - Target to trace (default: 8.8.8.8)

set -e

TTL=${TTL:-./target/release/ttl}
TARGET=${TARGET:-8.8.8.8}

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
NC='\033[0m'

pass() { echo -e "${GREEN}OK${NC}"; }
fail() { echo -e "${RED}FAIL${NC}"; exit 1; }

echo "=== ttl smoke tests ==="
echo "Binary: $TTL"
echo "Target: $TARGET"
echo ""

# Check binary exists
if [ ! -x "$TTL" ]; then
    echo "Error: $TTL not found or not executable"
    echo "Run: cargo build --release"
    exit 1
fi

# Version check
echo -n "Version... "
$TTL --version > /dev/null && pass || fail

# Help check
echo -n "Help... "
$TTL --help > /dev/null && pass || fail

# Basic invocation
echo -n "Basic trace (3 probes)... "
timeout 15 $TTL $TARGET -c 3 --json > /dev/null 2>&1 && pass || fail

# Protocol modes
for proto in icmp udp tcp; do
    echo -n "Protocol $proto... "
    timeout 15 $TTL -p $proto $TARGET -c 3 --json > /dev/null 2>&1 && pass || fail
done

# PMTUD (needs time for binary search - use faster interval)
echo -n "PMTUD mode... "
timeout 90 $TTL --pmtud $TARGET -c 50 -i 0.2 --json > /dev/null 2>&1 && pass || fail

# Multi-flow
echo -n "Multi-flow (4 flows)... "
timeout 20 $TTL --flows 4 $TARGET -c 5 --json > /dev/null 2>&1 && pass || fail

# DSCP marking
echo -n "DSCP marking... "
timeout 15 $TTL --dscp 46 $TARGET -c 3 --json > /dev/null 2>&1 && pass || fail

# Export formats
echo -n "CSV export... "
timeout 15 $TTL $TARGET -c 3 --csv > /dev/null 2>&1 && pass || fail

echo -n "Report export... "
timeout 15 $TTL $TARGET -c 3 --report > /dev/null 2>&1 && pass || fail

# Multiple targets
echo -n "Multiple targets... "
timeout 20 $TTL $TARGET 1.1.1.1 -c 3 --json > /dev/null 2>&1 && pass || fail

echo ""
echo -e "${GREEN}=== All smoke tests passed ===${NC}"
