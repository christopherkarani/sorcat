#!/usr/bin/env bash
set -euo pipefail

OUT_DIR="target/spec-evidence"
mkdir -p "${OUT_DIR}"

echo "[spec-evidence] running deterministic score/decompile captures"

cargo run -p sorcat-cli -- score > "${OUT_DIR}/score-run1.txt"
cargo run -p sorcat-cli -- score > "${OUT_DIR}/score-run2.txt"
cargo run -p sorcat-cli -- decompile fixtures/wasm/cfg_branch_loop_merge.wasm > "${OUT_DIR}/decompile-run1.txt"
cargo run -p sorcat-cli -- decompile fixtures/wasm/cfg_branch_loop_merge.wasm > "${OUT_DIR}/decompile-run2.txt"
cargo run -p sorcat-cli -- score --output "${OUT_DIR}/score-report.json" > "${OUT_DIR}/score-with-report.txt"

shasum -a 256 "${OUT_DIR}/score-run1.txt" "${OUT_DIR}/score-run2.txt" > "${OUT_DIR}/score-sha256.txt"
shasum -a 256 "${OUT_DIR}/decompile-run1.txt" "${OUT_DIR}/decompile-run2.txt" > "${OUT_DIR}/decompile-sha256.txt"

if cmp -s "${OUT_DIR}/score-run1.txt" "${OUT_DIR}/score-run2.txt"; then
  score_cmp="0"
else
  score_cmp="1"
fi

if cmp -s "${OUT_DIR}/decompile-run1.txt" "${OUT_DIR}/decompile-run2.txt"; then
  decompile_cmp="0"
else
  decompile_cmp="1"
fi

{
  echo "score_cmp_exit=${score_cmp}"
  echo "decompile_cmp_exit=${decompile_cmp}"
} > "${OUT_DIR}/determinism.txt"

if [[ "${score_cmp}" != "0" || "${decompile_cmp}" != "0" ]]; then
  echo "[spec-evidence] determinism check failed"
  exit 1
fi

echo "[spec-evidence] complete: ${OUT_DIR}"
