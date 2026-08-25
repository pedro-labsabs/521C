#!/usr/bin/env bash
# Fetch pinned, checksum-verified portable tools into .tools/bin.
# No global installs. Idempotent: skips tools already present and working.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/.tools/bin"
mkdir -p "$BIN"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

OS="$(uname -s)"
ARCH="$(uname -m)"
if [ "$OS" != "Linux" ]; then
  echo "fetch-tools: only Linux is supported" >&2
  exit 1
fi

case "$ARCH" in
  x86_64)
    RTK_TARGET="x86_64-unknown-linux-musl"
    RTK_SHA256="c4c036fbf181fc55ef329786c8c17e0d427972b053b825944d968a6aafef1ba4"
    JUST_TARGET="x86_64-unknown-linux-musl"
    JUST_SHA256="4a5cc2f53e6f0f8c59092a6cc38291eb729d46a7dd95d3ae582008881b84931d"
    ;;
  aarch64)
    RTK_TARGET="aarch64-unknown-linux-gnu"
    RTK_SHA256="80a746dd305ef944ff50ef011ae4ce3878dd5ba88dfe35d859d05498191637c3"
    JUST_TARGET="aarch64-unknown-linux-musl"
    JUST_SHA256="748237128c4c40cbdabc65e841d05ceba13cc23a91eaba395495894c1d9764df"
    ;;
  *)
    echo "fetch-tools: unsupported architecture $ARCH" >&2
    exit 1
    ;;
esac

# Pinned upstream releases (official checksums from each release).
RTK_VERSION="v0.45.0"
RTK_URL="https://github.com/rtk-ai/rtk/releases/download/${RTK_VERSION}/rtk-${RTK_TARGET}.tar.gz"
JUST_VERSION="1.58.0"
JUST_URL="https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-${JUST_TARGET}.tar.gz"

fetch_tarball() { # $1=name $2=url $3=sha256 $4=binary-name-inside-tarball $5=installed-name
  local name="$1" url="$2" sha="$3" inner="$4" dest="$BIN/$5"
  if [ -x "$dest" ] && "$dest" --version >/dev/null 2>&1; then
    echo "fetch-tools: $5 already present ($("$dest" --version 2>&1 | head -1))"
    return 0
  fi
  echo "fetch-tools: downloading $name"
  curl -fsSL --retry 3 -o "$TMP/$name" "$url"
  echo "$sha  $TMP/$name" | sha256sum -c - >/dev/null \
    || { echo "fetch-tools: CHECKSUM MISMATCH for $name" >&2; exit 1; }
  tar -xzf "$TMP/$name" -C "$TMP"
  install -m 0755 "$TMP/$inner" "$dest"
  echo "fetch-tools: installed $5 ($("$dest" --version 2>&1 | head -1))"
}

fetch_tarball "rtk-${RTK_VERSION}-${RTK_TARGET}.tar.gz" "$RTK_URL" "$RTK_SHA256" "rtk" "rtk"
fetch_tarball "just-${JUST_VERSION}-${JUST_TARGET}.tar.gz" "$JUST_URL" "$JUST_SHA256" "just" "just"
