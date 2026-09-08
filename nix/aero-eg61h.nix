# aero_eg61h — NixOS modülü
#
# `aorus_laptop`'ın yerini alır. O sürücüden farkı: yalnız ölçülmüş yetenekleri
# sunar, ve yazdığı her şeyi geri okuyup doğrular.
#
# Kullanım (~/nixos-zixar tarafında):
#
#   inputs.aero-eg61h = { url = "git+file:///home/zixar/aero-eg61h"; flake = false; };
#   ...
#   imports = [ "${inputs.aero-eg61h}/nix/aero-eg61h.nix" ];
#   hardware.aero-eg61h.enable = true;
#
# Bunu açtığınızda `system/arch/aerox16/wmi.nix`'teki şu üç servis ve
# `aorus-laptop` türetmesi KALDIRILMALI — ikisi aynı işi yapar ve sürücü zaten
# `aorus_laptop` yüklüyse bağlanmayı reddeder:
#   gigabyte-power-profile.service · gigabyte-charge-limit.service · fan-mode-cycle.service
# `acpi_call` ve dGPU ACBT bloğu KALIYOR (bu modül onları devralıyor).
{ config, lib, pkgs, ... }:

let
  cfg = config.hardware.aero-eg61h;
  kernel = config.boot.kernelPackages.kernel;

  # Derleme artıklarını store'a taşımayalım: aksi hâlde her yerel `./build.sh`
  # koşusu src hash'ini değiştirir ve modül gereksiz yere yeniden derlenir.
  aeroSrc = lib.cleanSourceWith {
    name = "aero-eg61h-kernel-src";
    src = ../kernel;
    filter = path: type:
      let base = baseNameOf (toString path); in
      !(lib.hasSuffix ".o" base
        || lib.hasSuffix ".ko" base
        || lib.hasSuffix ".mod" base
        || lib.hasSuffix ".mod.c" base
        || lib.hasSuffix ".cmd" base
        || base == "modules.order"
        || base == "Module.symvers"
        || base == ".tmp_versions");
  };

  # ARAÇ ZİNCİRİ: pkgs.stdenv DEĞİL, kernel.stdenv. CachyOS-lto çekirdeği
  # clang + LLVM=1 ile derlendi; gcc, kernel Makefile'ının verdiği
  # -mstack-alignment=8 / -fsplit-lto-unit bayraklarını tanımaz ve çöker.
  # (Aynı desen: nixpkgs'in kendi acpi_call türetmesi.)
  aero-eg61h = kernel.stdenv.mkDerivation {
    pname = "aero-eg61h";
    version = "0.1.0-unstable-2026-09-07";

    src = aeroSrc;

    nativeBuildInputs = kernel.moduleBuildDependencies;
    hardeningDisable = [ "pic" ];

    makeFlags = config.boot.kernelPackages.kernelModuleMakeFlags ++ [
      "KDIR=${kernel.dev}/lib/modules/${kernel.modDirVersion}/build"
    ];

    installPhase = ''
      runHook preInstall
      install -D aero-eg61h.ko \
        $out/lib/modules/${kernel.modDirVersion}/extra/aero-eg61h.ko
      runHook postInstall
    '';

    meta = with lib; {
      description = "Gigabyte AERO X16 1VH (EG61VH) platform driver";
      license = licenses.gpl2Only;
      platforms = platforms.linux;
    };
  };

  # Fan modu düğümünün yolu GUID içeriyor. Sondaki örnek indeksi (-2) _WDG
  # sırasından geliyor ve kararlı görünüyor, ama glob daha dayanıklı.
  fanModeNode = ''"$(echo /sys/bus/wmi/devices/ABBC0F75-*/fan_mode)"'';
  chargeNode = "/sys/class/power_supply/BAT1/charge_control_end_threshold";

  fanModes = [ "quiet" "balanced" "responsive" "gaming" "turbo" ];
in
{
  options.hardware.aero-eg61h = {
    enable = lib.mkEnableOption "Gigabyte AERO X16 1VH (EG61VH) platform sürücüsü";

    fanMode = {
      ac = lib.mkOption {
        type = lib.types.enum fanModes;
        default = "balanced";
        description = ''
          AC'de uygulanacak fan modu.

          Varsayılan `balanced` (PECM+0x2C = 0x09) BİLEREK seçildi: makine
          `aorus_laptop` altında `fan_mode = 1` ("sessiz") yazılıyken aslında
          bu modda koşuyordu (7 Eyl 2026 ölçümü). `quiet` yazmak davranışı
          DEĞİŞTİRİR — mod 4 sessiz gibi geç başlar (54 °C) ama varsayılan gibi
          yükselebilir (%43 tavan).
        '';
      };
      battery = lib.mkOption {
        type = lib.types.enum fanModes;
        default = "balanced";
        description = "Pilde uygulanacak fan modu.";
      };
      game = lib.mkOption {
        type = lib.types.enum fanModes;
        default = "turbo";
        description = ''
          Oyun oturumunda (`game-perf.service` aktifken) uygulanacak mod.
          `sched.nix` bunu kendisi yazıyor; burası yalnız belgeleme amaçlı.
        '';
      };
      cycle = lib.mkOption {
        type = lib.types.listOf (lib.types.enum fanModes);
        default = [ "balanced" "quiet" "gaming" "turbo" ];
        description = "Süper+M ile dönülecek mod sırası.";
      };
    };

    chargeLimit = lib.mkOption {
      type = lib.types.ints.between 1 100;
      default = 60;
      description = "Pil şarj limiti (%). Standart ABI üzerinden yazılır.";
    };

    gpuBoost = {
      ac = lib.mkOption {
        type = lib.types.ints.between 0 10;
        default = 10;
        description = "AC'de dGPU Dynamic Boost bütçesi (NPCF.ACBT = değer × 8 W).";
      };
      battery = lib.mkOption {
        type = lib.types.ints.between 0 10;
        default = 0;
        description = "Pilde dGPU Dynamic Boost bütçesi.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    boot.extraModulePackages = [ aero-eg61h config.boot.kernelPackages.acpi_call ];
    boot.kernelModules = [ "aero-eg61h" "acpi_call" ];

    # İkisi aynı WMI metotlarını çağırıyor. Sürücü çakışmayı kendi tespit edip
    # -EBUSY ile reddediyor, ama niyeti açık kılmak için blacklist.
    boot.blacklistedKernelModules = [ "aorus-laptop" ];

    # ---------------------------------------------------------------------
    # AC/BAT fan modu + dGPU boost bütçesi
    # ---------------------------------------------------------------------
    # udev olayıyla tetiklenir, yoklama YOK — güç katmanının deseni bu
    # (4.28 W boşta bütçesi).
    systemd.services.aero-power-profile = {
      description = "AC/BAT fan modu + dGPU boost bütçesi (aero_eg61h)";
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-modules-load.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "aero-power-profile" ''
          F=${fanModeNode}
          [ -w "$F" ] || exit 0
          AC=$(cat /sys/class/power_supply/ACAD/online 2>/dev/null || echo 1)
          if [ "$AC" = "0" ]; then
            FAN=${cfg.fanMode.battery}
            ACBT=${toString cfg.gpuBoost.battery}
          else
            FAN=${cfg.fanMode.ac}
            ACBT=${toString cfg.gpuBoost.ac}
          fi

          # OYUN İSTİSNASI: game-perf turbo yazmışken araya girip düşürmeyelim.
          # (10 Ağu 2026 dersi: eski servis bunu koşulsuz yapıyordu ve oyunun
          # ortasında fişle oynamak turbo'yu sessizce kesiyordu.)
          if ${pkgs.systemd}/bin/systemctl is-active --quiet game-perf.service; then
            :
          else
            echo "$FAN" > "$F" || echo "aero: fan modu yazilamadi" >&2
          fi

          # dGPU Dynamic Boost bütçesi hâlâ ham WMI: sürücü dGPU kollarını
          # SUNMUYOR (NPCF yazımı nvidia.ko ile yarışabilir, surucu-tasarim §3.8).
          if [ -w /proc/acpi/call ]; then
            echo "\\_SB.PCI0.AMW0.WMBD 0 0x4C $ACBT" > /proc/acpi/call
            cat /proc/acpi/call > /dev/null
          fi
        '';
      };
    };

    # ---------------------------------------------------------------------
    # Şarj limiti
    # ---------------------------------------------------------------------
    systemd.services.aero-charge-limit = {
      description = "Pil şarj limiti %%${toString cfg.chargeLimit} (aero_eg61h)";
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-modules-load.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "aero-charge-limit" ''
          # BCPS (charge_mode) sürücüde SUNULMUYOR: anlamı DSDT'den
          # doğrulanamıyor ve standart bir karşılığı yok. Ama şarj limitinin
          # etkili olması buna bağlı görünüyor, o yüzden ham WMI ile yazılıyor.
          # Değer 4 TAHMİN DEĞİL, ÖLÇÜM: aorus_laptop'ın charge_mode = 1'i
          # EC'de BCPS = 4 üretiyordu (WMBC 0x64 = 0x04, 7 Eyl 2026).
          if [ -w /proc/acpi/call ]; then
            echo '\_SB.PCI0.AMW0.WMBD 0 0x64 4' > /proc/acpi/call
            cat /proc/acpi/call > /dev/null
          fi

          # Standart ABI. Sürücü yazımı geri okuyup doğruluyor, uyuşmazlıkta
          # -EIO dönüyor — yani buradaki hata gerçek bir hata.
          [ -w ${chargeNode} ] || exit 0
          echo ${toString cfg.chargeLimit} > ${chargeNode}
        '';
      };
    };

    # ---------------------------------------------------------------------
    # Süper+M fan modu döngüsü
    # ---------------------------------------------------------------------
    systemd.services.aero-fan-cycle = {
      description = "aero_eg61h fan modunu döndür + masaüstü bildirimi";
      after = [ "systemd-modules-load.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "aero-fan-cycle" ''
          F=${fanModeNode}
          [ -w "$F" ] || exit 0
          cur=$(${pkgs.coreutils}/bin/cat "$F" 2>/dev/null || echo "")

          # Döngü listesi; bilinmeyen/`unknown` okuma ilk girdiye döner.
          set -- ${lib.concatStringsSep " " cfg.fanMode.cycle}
          first="$1"; next=""; found=""
          for m in "$@"; do
            if [ -n "$found" ]; then next="$m"; break; fi
            [ "$m" = "$cur" ] && found=1
          done
          [ -n "$next" ] || next="$first"

          echo "$next" > "$F" || exit 1

          # Ne yazdığımızı değil, EC'nin GERÇEKTEN ne koştuğunu bildir.
          # (aorus_laptop'ın hatası tam olarak buydu: yazdığını bildiriyordu.)
          shown=$(${pkgs.coreutils}/bin/cat "$F")
          uid=$(${pkgs.coreutils}/bin/id -u zixar 2>/dev/null || echo 1000)
          ${pkgs.util-linux}/bin/runuser -u zixar -- \
            ${pkgs.coreutils}/bin/env \
              DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" \
              XDG_RUNTIME_DIR="/run/user/$uid" \
            ${pkgs.libnotify}/bin/notify-send -a Fan -u low -t 2000 \
              "Fan modu" "$shown" >/dev/null 2>&1 || true
        '';
      };
    };

    # zixar, aero-fan-cycle.service'i şifresiz start edebilsin (Süper+M keybind)
    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        if (action.id == "org.freedesktop.systemd1.manage-units" &&
            action.lookup("unit") == "aero-fan-cycle.service" &&
            subject.user == "zixar") {
          return polkit.Result.YES;
        }
      });
    '';

    # AC/pil değişimi udev olayıyla yakalanır, zamanlayıcıyla değil.
    services.udev.extraRules = ''
      SUBSYSTEM=="power_supply", KERNEL=="ACAD", ATTR{online}=="*", \
        RUN+="${pkgs.systemd}/bin/systemctl --no-block start aero-power-profile.service"
    '';

    powerManagement.resumeCommands = ''
      ${pkgs.systemd}/bin/systemctl --no-block start aero-power-profile.service
    '';
  };
}
