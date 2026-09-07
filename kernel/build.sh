#!/usr/bin/env bash
# aero_eg61h'i çalışan çekirdeğe karşı derler.
#
# Araç zinciri ~/nixos-zixar flake'inden gelir — pkgs.stdenv DEĞİL,
# kernel.stdenv + kernelModuleMakeFlags. Gerekçe: CachyOS-lto çekirdeği
# clang-21 + LLVM=1 ile derlendi; gcc ile modül derlemesi çöker
# (-mstack-alignment=8 / -fsplit-lto-unit tanınmıyor).
# Aynı desen: ~/nixos-zixar/system/arch/aerox16/wmi.nix
#
# `nix run nixpkgs#…` KULLANILMIYOR: o, registry'nin nixpkgs'ini çözer, bu
# flake'in pinlediğini değil. make de flake'in kendi pkgs'inden alınıyor.
#
# nix build çağrılarında --out-link ŞART: --no-link GC kökü bırakmıyor ve
# nh'ın GC'si çekirdek dev çıktısını siliyor.
set -euo pipefail

FLAKE="${FLAKE:-$HOME/nixos-zixar}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GCROOTS="$(dirname "$HERE")/.gcroots"
KP="nixosConfigurations.nixos.config.boot.kernelPackages"
NIX="nix --no-warn-dirty"

mkdir -p "$GCROOTS"
cd "$FLAKE"

echo "==> cekirdek dev ciktisi (GC koku: $GCROOTS)"
$NIX build --out-link "$GCROOTS/kernel-dev" ".#$KP.kernel.dev"
KDEV="$(readlink -f "$GCROOTS/kernel-dev-dev" 2>/dev/null || readlink -f "$GCROOTS/kernel-dev")"

KVER="$(basename "$(echo "$KDEV"/lib/modules/*)")"
KDIR="$KDEV/lib/modules/$KVER/build"
[ -d "$KDIR" ] || { echo "HATA: $KDIR yok" >&2; exit 1; }

if [ "$KVER" != "$(uname -r)" ]; then
	echo "UYARI: flake cekirdegi $KVER, calisan cekirdek $(uname -r) — modul yuklenemez" >&2
fi

echo "==> make (flake'in kendi gnumake'i)"
$NIX build --out-link "$GCROOTS/gnumake" ".#nixosConfigurations.nixos.pkgs.gnumake"
MAKE="$GCROOTS/gnumake/bin/make"

echo "==> arac zinciri bayraklari"
mapfile -t FLAGS < <($NIX eval --raw ".#$KP.kernelModuleMakeFlags" \
	--apply 'builtins.concatStringsSep "\n"')

echo "==> derleniyor (KDIR=$KDIR)"
"$MAKE" -C "$KDIR" M="$HERE" "${FLAGS[@]}" modules

echo
echo "==> hazir: $HERE/aero-eg61h.ko"
