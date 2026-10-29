#!/bin/bash
# Etapas 38 e 42 (Skills e Memória). Uso a partir de desktop/: bash e2e/skills-memoria-rodar.sh
# Vite de teste (mocks do Tauri) na 4192 e Edge headless com CDP na 9348; só esses dois processos são parados no fim.
set -u
cd "$(dirname "$0")/.." || exit 9
OUT="e2e/evidencias-skills-38-memoria-42"
VITE_PORT=4192
CDP_PORT=9348
EDGE="/c/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"
mkdir -p "$OUT"
rm -rf "$OUT/edge-profile"

npx vite --config e2e/vite.telas.config.ts --port $VITE_PORT --strictPort > "$OUT/vite.log" 2>&1 &
VITE_PID=$!
"$EDGE" --headless=new --remote-debugging-port=$CDP_PORT --user-data-dir="$(cygpath -w "$PWD/$OUT/edge-profile")" about:blank > "$OUT/edge.log" 2>&1 &
EDGE_PID=$!

stop_own() {
  for pid in "$VITE_PID" "$EDGE_PID"; do
    winpid=$(cat "/proc/$pid/winpid" 2>/dev/null) && taskkill //PID "$winpid" //T //F > /dev/null 2>&1
  done
}
trap stop_own EXIT

for _ in $(seq 1 60); do
  curl -s -m 2 "http://127.0.0.1:$CDP_PORT/json/list" | grep -q webSocketDebuggerUrl && curl -s -m 2 "http://localhost:$VITE_PORT/" > /dev/null && break
  sleep 1
done

SKILLS_CDP_PORT=$CDP_PORT SKILLS_APP_URL="http://localhost:$VITE_PORT/" node e2e/skills-memoria.cdp.mjs "$OUT" | tee "$OUT/run.log"
exit "${PIPESTATUS[0]}"
