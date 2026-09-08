# `aero-control` — COSMIC yerlisi GUI paketi.
#
# libcosmic crates.io'da YOK, git bağımlılığı. Bu yüzden `cargoLock.outputHashes`
# şart: her git bağımlılığının çıktısı ayrıca hash'lenmeli. Hash'i öğrenmenin
# yolu bir kez yanlış hash'le derleyip nix'in söylediğini almak
# (`got: sha256-…`) — aşağıdaki değer öyle alındı.
#
# `libcosmicAppHook` nixpkgs'in COSMIC uygulamaları için hazırladığı kanca:
# wrapper'a XDG_DATA_DIRS, ikon teması ve wayland/xkb kütüphane yollarını
# ekliyor. Elle wrapProgram yazmaya gerek yok.
{ lib
, rustPlatform
, pkg-config
, libcosmicAppHook
, just ? null
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "aero-control";
  version = "0.1.0";

  # DİKKAT: `../app` store'a "app" adıyla kopyalanıyor — o dizin TAŞINABİLİR
  # ama YENİDEN ADLANDIRILAMAZ (adı derivation adına giriyor).
  src = lib.cleanSourceWith {
    name = "aero-control-src";
    src = ../app;
    filter = path: type:
      let base = baseNameOf (toString path); in
      base != "target";
  };

  cargoLock = {
    lockFile = ../app/Cargo.lock;
    outputHashes = {
      # `nix build` bir kez yanlış hash'le koşturulup çıktısından alınacak.
      # Doldurulmadan bu türetme DERLENMEZ — bilerek: uydurma bir hash
      # koymaktansa açıkça başarısız olsun.
      "libcosmic-1.0.0" = lib.fakeHash;
    };
  };

  nativeBuildInputs = [ pkg-config libcosmicAppHook ];

  # Yalnız GUI'yi derle: aero-ctl ve aero-sysfs ayrı (ve libcosmic'siz).
  cargoBuildFlags = [ "-p" "aero-control" ];
  cargoTestFlags = finalAttrs.cargoBuildFlags;

  meta = {
    description = "Gigabyte AERO X16 1VH (EG61VH) için COSMIC yerlisi kontrol arayüzü";
    homepage = "https://github.com/zixar/aero-eg61h";
    license = lib.licenses.gpl2Only;
    mainProgram = "aero-control";
    platforms = lib.platforms.linux;
  };
})
