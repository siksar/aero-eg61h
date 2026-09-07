#!/usr/bin/env bash
# app/ workspace'ini derler ve testleri koşar.
#
# PATH'teki `cargo` bir rustup kabuğu ve bu makinede varsayılan toolchain YOK
# ("rustup could not choose a version of cargo to run"). Bu yüzden araç zinciri
# ~/nixos-zixar flake'inden çekiliyor — kernel/build.sh ile aynı gerekçe.
#
# `nix run nixpkgs#…` KULLANILMIYOR: o registry'nin nixpkgs'ini çözer,
# bu flake'in pinlediğini değil.
#
# nix build çağrılarında --out-link ŞART: --no-link GC kökü bırakmıyor ve
# nh'ın GC'si araçları siliyor.
set -euo pipefail

FLAKE="${FLAKE:-$HOME/nixos-zixar}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GCROOTS="$(dirname "$HERE")/.gcroots"
P="nixosConfigurations.nixos.pkgs"
NIX="nix --no-warn-dirty"

mkdir -p "$GCROOTS"
cd "$FLAKE"
for t in "rust:rustPlatform.rust.cargo" "rustc:rustPlatform.rust.rustc" "cc:stdenv.cc"; do
	name="${t%%:*}"; attr="${t#*:}"
	[ -e "$GCROOTS/$name" ] || $NIX build --out-link "$GCROOTS/$name" ".#$P.$attr"
done

export PATH="$GCROOTS/rust/bin:$GCROOTS/rustc/bin:$GCROOTS/cc/bin:$PATH"
cd "$HERE"

echo "==> test"
cargo test --offline

echo "==> release derleme"
cargo build --release --offline

echo
echo "==> hazir:"
ls -1 "$HERE"/target/release/aero-* 2>/dev/null | grep -v '\.d$' | sed 's/^/    /'
