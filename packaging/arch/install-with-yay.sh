#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

if ! command -v yay >/dev/null 2>&1; then
  echo "error: 找不到 yay，請先安裝 yay。"
  exit 1
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "error: 找不到 curl，請先安裝 curl。"
  exit 1
fi

latest_tag="$(
  curl -fsSL "https://api.github.com/repos/kawayiYokami/Easy-call-ai/releases/latest" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"v\([^"]*\)".*/\1/p' \
    | head -n 1
)"

if [[ -n "${latest_tag}" ]]; then
  sed -i -E "s/^pkgver=.*/pkgver=${latest_tag}/" PKGBUILD
  echo "info: 已將 PKGBUILD 更新到 pkgver=${latest_tag}"
else
  echo "warn: 無法取得最新版本，使用 PKGBUILD 目前版本。"
fi

echo "info: 開始使用 yay 建置並安裝 easy-call-ai..."
yay -Bi --needed "$@" .
