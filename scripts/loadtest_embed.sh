#!/usr/bin/env bash
# Load-tests the embedding path via `bible-mcp bench-embed`, one OS process per
# simulated concurrent tool call (matches how each MCP client session runs its
# own `bible-mcp serve` process today, model included).
#
# Usage:
#   ./scripts/loadtest_embed.sh            # concurrency levels 1 10 25 50
#   ./scripts/loadtest_embed.sh 10 100 200
#
# To instead measure concurrent calls *within* one already-running process
# (the shared Mutex<TextEmbedding> case), skip this script and run e.g.:
#   ./target/release/bible-mcp bench-embed --concurrency 50

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

LEVELS=("$@")
if [ "${#LEVELS[@]}" -eq 0 ]; then
    LEVELS=(1 10 25 50)
fi

BIN="$REPO_ROOT/target/release/bible-mcp"
if [ ! -x "$BIN" ]; then
    echo "Building release binary..."
    cargo build --release
fi

echo "Warming model cache..."
"$BIN" bench-embed "warm up" >/dev/null

RESULTS_DIR=$(mktemp -d)
trap 'rm -rf "$RESULTS_DIR"' EXIT

for N in "${LEVELS[@]}"; do
    echo
    echo "=== concurrency: $N ==="
    OUT="$RESULTS_DIR/n$N.txt"
    SAMPLES="$RESULTS_DIR/n${N}_samples.txt"
    : >"$OUT"
    : >"$SAMPLES"

    (
        while true; do
            ps -C bible-mcp -o rss=,pcpu= 2>/dev/null |
                awk '{rss+=$1; cpu+=$2} END {if (NR>0) print rss, cpu}' >>"$SAMPLES"
            sleep 0.2
        done
    ) &
    SAMPLER_PID=$!

    WALL_START=$(date +%s.%N)
    pids=()
    for i in $(seq 1 "$N"); do
        "$BIN" bench-embed "load test query $i about love and faith" >>"$OUT" &
        pids+=("$!")
    done
    for p in "${pids[@]}"; do wait "$p"; done
    WALL_END=$(date +%s.%N)

    kill "$SAMPLER_PID" 2>/dev/null || true
    wait "$SAMPLER_PID" 2>/dev/null || true

    WALL_MS=$(awk -v a="$WALL_START" -v b="$WALL_END" 'BEGIN{printf "%.1f", (b-a)*1000}')

    sort -n "$OUT" >"$RESULTS_DIR/n${N}_sorted.txt"
    awk -v wall_ms="$WALL_MS" -v n="$N" '
        { a[NR] = $1; sum += $1 }
        END {
            if (NR == 0) { print "no samples"; exit }
            p50 = a[int(0.50 * (NR - 1)) + 1]
            p95 = a[int(0.95 * (NR - 1)) + 1]
            printf "calls=%d/%d wall_ms=%s min=%.1f p50=%.1f p95=%.1f max=%.1f mean=%.1f\n", \
                NR, n, wall_ms, a[1], p50, p95, a[NR], sum / NR
        }' "$RESULTS_DIR/n${N}_sorted.txt"

    awk '{rss=$1; cpu=$2; if (rss>maxrss) maxrss=rss; if (cpu>maxcpu) maxcpu=cpu}
         END {printf "peak_rss_mb=%.1f peak_cpu_pct=%.1f\n", maxrss/1024, maxcpu}' "$SAMPLES"
done
