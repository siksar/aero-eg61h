#!/usr/bin/env bash
# GUI'yi calistirir.
#
# NEDEN AYRI BIR BETIK: ikili dogrudan calistirilinca ANINDA panikliyor —
#
#   Create event loop: Os(... error: NoWaylandLib)
#
# winit `libwayland-client.so`'yu calisma aninda `dlopen` ediyor; Nix disinda
# derlenmis bir ikilinin rpath'inde o yok ve NixOS'ta /usr/lib de yok. Cozum
# LD_LIBRARY_PATH.
#
# Bu tuzak 8 Eyl 2026'da ikinci kez bulundu, cunku hicbir yerde yaziyla
# durmuyordu: `.gcroots/gui-env` vardi ama nasil kuruldugu da nicin gerektigi
# de yazili degildi. Simdi ikisi de burada, ve calistirilabilir bicimde.
#
# gui-env flake'te bir attribute DEGIL, elle kurulmus bir buildEnv — asagidaki
# ifade 8 Eyl 2026'da kurulup uygulama onunla CALISTIRILARAK dogrulandi.
#
# build.sh ile ayni kurallar: --out-link ZORUNLU (--no-link GC koku birakmiyor,
# nh'in GC'si araclari siliyor), `nix run nixpkgs#…` YASAK (registry'nin
# nixpkgs'ini cozer, flake'in pinini degil).
set -euo pipefail

FLAKE="${FLAKE:-$HOME/nixos-zixar}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GCROOTS="$(dirname "$HERE")/.gcroots"
BIN="${BIN:-$HERE/target/release/aero-control}"

if [ ! -e "$GCROOTS/gui-env" ]; then
	echo "==> gui-env yok, kuruluyor (bir kez)" >&2
	mkdir -p "$GCROOTS"
	nix --no-warn-dirty build --impure --out-link "$GCROOTS/gui-env" --expr "
	  let pkgs = (builtins.getFlake \"$FLAKE\").nixosConfigurations.nixos.pkgs;
	  in pkgs.buildEnv {
	    name = \"aero-gui-env\";
	    paths = with pkgs; [
	      wayland wayland-protocols libxkbcommon libglvnd mesa vulkan-loader
	      fontconfig freetype expat libinput systemdLibs pkg-config
	    ];
	    extraOutputsToInstall = [ \"dev\" \"lib\" ];
	  }"
fi

[ -x "$BIN" ] || { echo "HATA: $BIN yok — once ./build.sh" >&2; exit 1; }

export LD_LIBRARY_PATH="$GCROOTS/gui-env/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$BIN" "$@"
