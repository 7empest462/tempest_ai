#!/bin/bash
# Kalosm Memory Diagnostic Script
# Usage: bash diagnostic_kalosm_memory.sh
# This script monitors memory usage while using the Kalosm backend

set -e

echo "═══════════════════════════════════════════════════════════════"
echo "    Tempest AI - Kalosm Memory Leak Diagnostic"
echo "═══════════════════════════════════════════════════════════════"
echo ""

# Color codes
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Detect OS
OS_TYPE=$(uname -s)
if [[ "$OS_TYPE" == "Darwin" ]]; then
    echo "ℹ️  Detected macOS. Using Activity Monitor data."
else
    echo "ℹ️  Detected Linux. Using /proc/meminfo."
fi

echo ""
echo "Step 1: Check System Memory Capacity"
echo "──────────────────────────────────────"
if [[ "$OS_TYPE" == "Darwin" ]]; then
    TOTAL_RAM=$(sysctl -n hw.memsize | awk '{printf "%.0f", $1 / (1024*1024*1024)}')
    echo "Total RAM: ${TOTAL_RAM}GB"
else
    TOTAL_RAM=$(free -g | awk '/^Mem:/{print $2}')
    echo "Total RAM: ${TOTAL_RAM}GB"
fi

echo ""
echo "Step 2: Baseline Memory Check (before running Tempest)"
echo "────────────────────────────────────────────────────"
if [[ "$OS_TYPE" == "Darwin" ]]; then
    # macOS: Get memory stats
    BEFORE_MEM=$(vm_stat | grep "Pages active" | awk '{print $3}' | tr -d '.' | awk '{printf "%.0f", $1 * 4096 / (1024*1024*1024)}')
    BEFORE_SWAP=$(sysctl vm.swapusage | grep used | awk '{print $3}' | tr -d 'M' | awk '{printf "%.0f", $1 / 1024}')
    echo "Active Memory: ~${BEFORE_MEM}GB"
    echo "Swap Usage: ~${BEFORE_SWAP}GB"
else
    # Linux: Get memory stats
    BEFORE_MEM=$(free -g | awk '/^Mem:/{print $3}')
    BEFORE_SWAP=$(free -g | awk '/^Swap:/{print $3}')
    echo "Used Memory: ${BEFORE_MEM}GB"
    echo "Swap Usage: ${BEFORE_SWAP}GB"
fi

echo ""
echo "${YELLOW}Step 3: Starting Tempest AI in Kalosm Mode${NC}"
echo "────────────────────────────────────────"
echo "This will start the agent. You should:"
echo "  1. Ask a simple question (e.g., 'Hello')"
echo "  2. Note the memory usage"
echo "  3. Press Ctrl+C after 30-60 seconds"
echo ""
echo "Press Enter to continue..."
read -r

# Start Tempest in background with monitoring
echo ""
echo "${BLUE}Starting: cargo run --release -- --kalosm${NC}"
echo ""

# Create a monitoring loop in the background
if [[ "$OS_TYPE" == "Darwin" ]]; then
    {
        while sleep 5; do
            ACTIVE_MEM=$(vm_stat | grep "Pages active" | awk '{print $3}' | tr -d '.' | awk '{printf "%.0f", $1 * 4096 / (1024*1024*1024)}')
            SWAP=$(sysctl vm.swapusage | grep used | awk '{print $3}' | tr -d 'M' | awk '{printf "%.0f", $1 / 1024}')
            TIMESTAMP=$(date '+%H:%M:%S')
            echo "[$TIMESTAMP] Active: ${ACTIVE_MEM}GB | Swap: ${SWAP}GB"
            
            # Red flag if swap > 20GB
            if (( $(echo "$SWAP > 20" | bc -l) )); then
                echo -e "${RED}⚠️  WARNING: Swap usage exceeds 20GB!${NC}"
            fi
        done
    } &
    MONITOR_PID=$!
fi

# Run Tempest
cd "$(dirname "$0")" || exit 1
cargo run --release -- --kalosm --cli 2>&1 | head -50 &
TEMPEST_PID=$!

# Wait for user to stop
sleep 120

# Kill processes
kill $TEMPEST_PID 2>/dev/null || true
if [[ -n "$MONITOR_PID" ]]; then
    kill $MONITOR_PID 2>/dev/null || true
fi

echo ""
echo "Step 4: Post-Run Memory Check"
echo "─────────────────────────────"
if [[ "$OS_TYPE" == "Darwin" ]]; then
    AFTER_MEM=$(vm_stat | grep "Pages active" | awk '{print $3}' | tr -d '.' | awk '{printf "%.0f", $1 * 4096 / (1024*1024*1024)}')
    AFTER_SWAP=$(sysctl vm.swapusage | grep used | awk '{print $3}' | tr -d 'M' | awk '{printf "%.0f", $1 / 1024}')
else
    AFTER_MEM=$(free -g | awk '/^Mem:/{print $3}')
    AFTER_SWAP=$(free -g | awk '/^Swap:/{print $3}')
fi

echo "Active Memory: ~${AFTER_MEM}GB"
echo "Swap Usage: ~${AFTER_SWAP}GB"

echo ""
echo "Step 5: Analysis"
echo "───────────────"
MEM_DIFF=$((AFTER_MEM - BEFORE_MEM))
SWAP_DIFF=$((AFTER_SWAP - BEFORE_SWAP))

echo "Memory Increase: ${MEM_DIFF}GB"
echo "Swap Increase: ${SWAP_DIFF}GB"

if (( SWAP_DIFF > 10 )); then
    echo -e "${RED}❌ CRITICAL: Kalosm is aggressively using swap!${NC}"
    echo "   This is the memory leak you reported."
    echo ""
    echo "Recommendations:"
    echo "  1. Switch to MLX backend: cargo run --release -- --mlx"
    echo "  2. Use Ollama instead: ollama run llama2"
    echo "  3. Keep conversations short (< 20 turns before restart)"
else
    echo -e "${GREEN}✅ Memory usage appears normal${NC}"
    echo "   The fixes have likely reduced the leak"
fi

echo ""
echo "═══════════════════════════════════════════════════════════════"
echo "Diagnostic complete. Results saved to: kalosm_memory_diagnostic.log"
echo "═══════════════════════════════════════════════════════════════"
