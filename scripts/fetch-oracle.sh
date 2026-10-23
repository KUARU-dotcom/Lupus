#!/usr/bin/env bash
# Скачивает эталонный бинарник Lupus v0.4.0 для дифференциального теста.
# Использование: scripts/fetch-oracle.sh
# Переопределение ассета: LUPUS_ORACLE_ASSET=... scripts/fetch-oracle.sh
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dest="$root/oracles/v0.4.0"
asset="${LUPUS_ORACLE_ASSET:-lupus-v0.4.0-linux-x86_64.tar.gz}"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

mkdir -p "$dest"
echo "Скачивание $asset (release v0.4.0)..."
gh release download v0.4.0 --pattern "$asset" --dir "$tmp" --clobber

case "$asset" in
  *.tar.gz|*.tgz)
    tar -xzf "$tmp/$asset" -C "$tmp"
    ;;
  *.zip)
    if command -v unzip >/dev/null 2>&1; then
      unzip -q "$tmp/$asset" -d "$tmp"
    else
      echo "Для zip-ассета нужен unzip" >&2
      exit 1
    fi
    ;;
  *)
    echo "Неизвестный формат ассета: $asset" >&2
    exit 1
    ;;
esac

if [[ ! -f "$tmp/lupus" ]]; then
  echo "В архиве не найден бинарник lupus" >&2
  exit 1
fi

install -m 755 "$tmp/lupus" "$dest/lupus"
echo "Готово: $dest/lupus"
"$dest/lupus" --version
