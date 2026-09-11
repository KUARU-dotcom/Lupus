#!/usr/bin/env bash
# Сборка релизных бинарников Lupus из тегового коммита и упаковка дистрибутивов.
#
# Использование:
#   ./build_release.sh [VERSION]        # VERSION по умолчанию — текущий Cargo.toml version
#   ./build_release.sh 0.3.1
#
# Результат (в dist/):
#   lupus-v<VERSION>-linux-x86_64.tar.gz
#   lupus-v<VERSION>-windows-x86_64.zip
#   SHA256SUMS
set -euo pipefail

project_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$project_root"

# ─── Обязательные инструменты ─────────────────────────────────────────────────
for tool in cargo rustup zip; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Ошибка: %s не найден. Установите его и повторите.\n' "$tool" >&2
        exit 1
    fi
done

if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
    printf '%s\n' 'Ошибка: mingw (x86_64-w64-mingw32-gcc) не найден. Установите mingw-w64-gcc.' >&2
    exit 1
fi

# ─── Версия ───────────────────────────────────────────────────────────────────
VERSION="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)}"
if [ -z "$VERSION" ]; then
    printf '%s\n' 'Ошибка: не удалось определить версию. Укажите её аргументом: ./build_release.sh <version>' >&2
    exit 1
fi
TAG="v${VERSION}"

# ─── Собираем строго из тегового коммита ──────────────────────────────────────
if git rev-parse -q --verify "refs/tags/$TAG" >/dev/null; then
    git checkout --detach "$TAG"
else
    printf 'Ошибка: тег %s не найден локально.\n' "$TAG" >&2
    exit 1
fi

# ─── Цели кросс-компиляции ────────────────────────────────────────────────────
rustup target add x86_64-unknown-linux-gnu x86_64-pc-windows-gnu

echo ">> Сборка Linux (x86_64)..."
cargo build --release --target x86_64-unknown-linux-gnu
echo ">> Сборка Windows (x86_64-pc-windows-gnu)..."
cargo build --release --target x86_64-pc-windows-gnu

# ─── Упаковка ─────────────────────────────────────────────────────────────────
mkdir -p dist
TARBALL="dist/lupus-v${VERSION}-linux-x86_64.tar.gz"
ZIPFILE="dist/lupus-v${VERSION}-windows-x86_64.zip"

echo ">> Упаковка дистрибутивов..."
rm -rf dist/_pack && mkdir -p dist/_pack/linux dist/_pack/windows
cp target/x86_64-unknown-linux-gnu/release/lupus  dist/_pack/linux/lupus
cp LICENSE dist/_pack/linux/
cp README.md dist/_pack/linux/
cp target/x86_64-pc-windows-gnu/release/lupus.exe dist/_pack/windows/lupus.exe
cp LICENSE dist/_pack/windows/
cp README.md dist/_pack/windows/

tar -C dist/_pack/linux -czf "$TARBALL" lupus LICENSE README.md
( cd dist/_pack/windows && zip -q "$project_root/$ZIPFILE" lupus.exe LICENSE README.md )

echo ">> Генерация SHA256SUMS..."
( cd dist && sha256sum lupus-v*.tar.gz lupus-v*.zip > SHA256SUMS )

rm -rf dist/_pack

# ─── Возврат на исходную ветку ───────────────────────────────────────────────
git checkout - >/dev/null 2>&1 || git checkout main >/dev/null 2>&1

printf '\nГотово:\n'
printf '  Linux:   %s/%s\n' "$project_root" "$TARBALL"
printf '  Windows: %s/%s\n' "$project_root" "$ZIPFILE"
printf '  Хеши:   %s/SHA256SUMS\n' "$project_root"
