#!/usr/bin/env bash
# UECM one-shot deploy: mac → lanPC.
# Builds nothing locally; ships source + triggers tauri build on lanPC,
# then copies the produced uecm.exe into C:\Tools\UECM.

set -euo pipefail

SSH_HOST="${UECM_DEPLOY_HOST:-lanpc}"
REMOTE_STAGING='E:\uecm-plan4-test\.deploy-staging'
REMOTE_STAGING_FWDSLASH='E:/uecm-plan4-test/.deploy-staging'

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

REMOTE_PS1_LOCAL="$ROOT/scripts/deploy-lanpc-remote.ps1"
if [[ ! -f "$REMOTE_PS1_LOCAL" ]]; then
  echo "missing $REMOTE_PS1_LOCAL" >&2
  exit 1
fi

say() { printf '\033[1;36m[mac]\033[0m %s\n' "$*"; }
ok()  { printf '\033[1;32m[mac]\033[0m %s\n' "$*"; }
die() { printf '\033[1;31m[mac]\033[0m %s\n' "$*" >&2; exit 1; }

say "=== UECM deploy → $SSH_HOST ==="

# 1. Local typecheck (fail fast)
say "[1/5] mac: pnpm typecheck"
pnpm typecheck

# 2. Build tar (exclude heavy / irrelevant trees)
say "[2/5] mac: build tar"
TAR_FILE="$(mktemp -t uecm-deploy.XXXXXX).tar.gz"
trap 'rm -f "$TAR_FILE"' EXIT

COPYFILE_DISABLE=1 tar -czf "$TAR_FILE" \
  --exclude='./.git' \
  --exclude='./node_modules' \
  --exclude='./src-tauri/target' \
  --exclude='./dist' \
  --exclude='./.claude' \
  --exclude='./.agents' \
  --exclude='./.superpowers' \
  --exclude='./.claire' \
  --exclude='./.history' \
  --exclude='./.DS_Store' \
  --exclude='./coverage' \
  --exclude='./docs' \
  --exclude='*.tar.gz' \
  --exclude='*.jpeg' \
  --exclude='*.tsbuildinfo' \
  --exclude='./skills-lock.json' \
  --exclude='./vite.config.js' \
  --exclude='./vite.config.d.ts' \
  --exclude='./vitest.config.js' \
  --exclude='./vitest.config.d.ts' \
  -C "$ROOT" .

TAR_BYTES=$(wc -c <"$TAR_FILE" | tr -d ' ')
say "       tar size: $(( TAR_BYTES / 1024 / 1024 )) MB"

# 3. Prepare remote staging dir
say "[3/5] lanPC: prepare staging dir"
ssh "$SSH_HOST" "powershell -NoProfile -Command \"New-Item -ItemType Directory -Force -Path '$REMOTE_STAGING' | Out-Null\""

# 4. scp tar + ps1
say "[4/5] lanPC: scp tar + ps1"
scp -q "$TAR_FILE"          "$SSH_HOST:$REMOTE_STAGING_FWDSLASH/deploy.tar.gz"
scp -q "$REMOTE_PS1_LOCAL"  "$SSH_HOST:$REMOTE_STAGING_FWDSLASH/deploy-lanpc-remote.ps1"

# 5. Trigger remote deploy
say "[5/5] lanPC: run remote deploy (this runs typecheck/install/build/deploy)"
ssh "$SSH_HOST" "powershell -NoProfile -ExecutionPolicy Bypass -File $REMOTE_STAGING\\deploy-lanpc-remote.ps1"

ok "=== deploy done. Launch C:\\Tools\\UECM\\uecm.exe (GUI) + uecm-cli.exe (CLI) to verify. ==="
