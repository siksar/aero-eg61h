# aero_eg61h — NixOS modülü
#
# It exposes only measured capabilities and verifies every write by reading the
# corresponding EC state back.
#
# Kullanım (~/nixos-zixar tarafında):
#
#   inputs.aero-eg61h = { url = "git+file:///home/zixar/aero-eg61h"; flake = false; };
#   ...
#   imports = [ "${inputs.aero-eg61h}/nix/aero-eg61h.nix" ];
#   hardware.aero-eg61h.enable = true;
#
# Enable this module instead of any other driver or service that writes the same
# EC WMI methods. Every EC write goes through the kernel driver; `acpi_call` is
# no longer loaded (it bypassed the driver's lock and DMI gate and allowed root
# to call arbitrary ACPI methods).
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
  gpuBoostNode = ''"$(echo /sys/bus/wmi/devices/ABBC0F75-*/dgpu_boost)"'';
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
          The measured firmware table uses `balanced` as the reference default.
          `quiet` changes the thermal curve and starts later at 54 °C.
        '';
      };
      battery = lib.mkOption {
        type = lib.types.enum fanModes;
        default = "balanced";
        description = "Fan mode to apply on battery.";
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
        description = "Fan-mode cycle used by the Super+M shortcut.";
      };
    };

    user = lib.mkOption {
      type = lib.types.str;
      default = "zixar";
      description = "User allowed to change fan mode and charge limit without a password.";
    };

    chargeLimit = lib.mkOption {
      type = lib.types.ints.between 1 100;
      default = 60;
      description = "Battery charge limit (%), written through the standard ABI.";
    };

    gpuBoost = {
      ac = lib.mkOption {
        type = lib.types.ints.between 0 10;
        default = 10;
        description = "dGPU Dynamic Boost budget on AC (NPCF.ACBT = value × 8 W).";
      };
      battery = lib.mkOption {
        type = lib.types.ints.between 0 10;
        default = 0;
        description = "dGPU Dynamic Boost budget on battery.";
      };
    };
  };

  config = lib.mkIf cfg.enable {
    boot.extraModulePackages = [ aero-eg61h ];
    boot.kernelModules = [ "aero-eg61h" ];

    # Prevent a second EC writer from claiming the same WMI methods.
    boot.blacklistedKernelModules = [ "aorus-laptop" ];

    # ---------------------------------------------------------------------
    # AC/BAT fan modu + dGPU boost bütçesi
    # ---------------------------------------------------------------------
    # udev olayıyla tetiklenir, yoklama YOK — güç katmanının deseni bu
    # (4.28 W boşta bütçesi).
    systemd.services.aero-power-profile = {
      description = "AC/BAT fan mode + dGPU boost budget (aero_eg61h)";
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
            echo "$FAN" > "$F" || echo "aero: could not write fan mode" >&2
          fi

          # dGPU Dynamic Boost bütçesi (WMBD 0x4C) artık sürücünün
          # `dgpu_boost` düğümünden, io_lock altında yazılıyor. EC tarafında
          # geri okuma yok; hata yalnız ACPI değerlendirmesinin kendisinden.
          B=${gpuBoostNode}
          if [ -w "$B" ]; then
            echo "$ACBT" > "$B" || echo "aero: could not write dGPU boost" >&2
          fi
        '';
      };
    };

    # ---------------------------------------------------------------------
    # Şarj limiti
    # ---------------------------------------------------------------------
    systemd.services.aero-charge-limit = {
      description = "Battery charge limit %%${toString cfg.chargeLimit} (aero_eg61h)";
      wantedBy = [ "multi-user.target" ];
      after = [ "systemd-modules-load.service" ];
      serviceConfig = {
        Type = "oneshot";
        ExecStart = pkgs.writeShellScript "aero-charge-limit" ''
          # BCPS (WMBD 0x64 = 4) artık sürücü tarafından, limit yazılırken
          # aynı kilit altında kurulup geri okunuyor (aero-battery.c).

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
      description = "Cycle the aero_eg61h fan mode and notify the desktop";
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
          # (the legacy driver's behavior tam olarak buydu: yazdığını bildiriyordu.)
          shown=$(${pkgs.coreutils}/bin/cat "$F")
          uid=$(${pkgs.coreutils}/bin/id -u ${cfg.user} 2>/dev/null || echo 1000)
          ${pkgs.util-linux}/bin/runuser -u ${cfg.user} -- \
            ${pkgs.coreutils}/bin/env \
              DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$uid/bus" \
              XDG_RUNTIME_DIR="/run/user/$uid" \
            ${pkgs.libnotify}/bin/notify-send -a Fan -u low -t 2000 \
              "Fan modu" "$shown" >/dev/null 2>&1 || true
        '';
      };
    };

    # ---------------------------------------------------------------------
    # YAZMA KÖPRÜSÜ — parametreli şablon birimler
    # ---------------------------------------------------------------------
    # KARAR (8 Eyl 2026, ÖLÇÜMLE): D-Bus daemon YOK.
    #
    # PLAN §2 daemon'u iki gerekçeyle koymuştu. Biri — GUI kapalıyken şarj
    # limitini uyanışta yeniden uygulamak — sürücünün kendi .resume kancasına
    # taşındı ve doğrulandı. Kalan tek gerekçe yetki aracılığıydı; ölçüldü ki
    # onun için zaten iki köprü var ve ikisi de YETKİSİZ kullanıcıyla çalışıyor:
    #
    #   powerprofilesctl set power-saver  ->  platform_profile = low-power
    #   systemctl start aero-fan-cycle    ->  fan modu değişti
    #
    # Yani profil için PPD kullanılıyor (hiç yeni kod yok), fan modu ve şarj
    # limiti için aynı polkit deseni parametreli hâle getiriliyor. Daemon
    # kurmak boşta güç bütçesi açısından da gereksiz bir yük olurdu.
    #
    # GÜVENLİK: şablonun %i'si kullanıcıdan geliyor, o yüzden birim betiği
    # girdiyi KENDİ doğruluyor. Doğrulamayı polkit kuralına bırakmak yanlış
    # olurdu — kural yalnız "bu birimi başlatabilir mi"yi biliyor, "hangi
    # değerle"yi değil.
    systemd.services."aero-set-fan@" = {
      description = "Set fan mode to %i (aero_eg61h)";
      serviceConfig.Type = "oneshot";
      scriptArgs = "%i";
      script = ''
        mode="$1"
        # Beyaz liste: sysfs'e yalnız bilinen beş isimden biri gider.
        case "$mode" in
          ${lib.concatStringsSep "|" fanModes}) ;;
          *) echo "invalid fan mode: $mode" >&2; exit 1 ;;
        esac
        F=$(echo /sys/bus/wmi/devices/ABBC0F75-*/fan_mode)
        [ -w "$F" ] || { echo "fan_mode node is missing — is the driver loaded?" >&2; exit 1; }
        echo "$mode" > "$F"
        # Sürücü yazımı zaten geri okuyup doğruluyor; biz de bakalım.
        got=$(cat "$F")
        [ "$got" = "$mode" ] || { echo "wrote $mode but read back $got" >&2; exit 1; }
      '';
    };

    systemd.services."aero-set-charge@" = {
      description = "Set charge limit to %i (aero_eg61h)";
      serviceConfig.Type = "oneshot";
      scriptArgs = "%i";
      script = ''
        pct="$1"
        case "$pct" in
          ""|*[!0-9]*) echo "not a number: $pct" >&2; exit 1 ;;
        esac
        # Sürücü 0'ı zaten reddediyor (anlamı ölçülmedi); burada da durduruyoruz
        # ki geçersiz değer sysfs'e hiç gitmesin.
        [ "$pct" -ge 1 ] && [ "$pct" -le 100 ] || { echo "outside 1-100: $pct" >&2; exit 1; }
        N=/sys/class/power_supply/BAT1/charge_control_end_threshold
        [ -w "$N" ] || { echo "charge-limit node is missing" >&2; exit 1; }
        echo "$pct" > "$N"
      '';
    };

    # Yazma köprüsünün polkit tarafı. Yalnız YEREL ve AKTİF oturum için
    # ŞİFRESİZ — PLAN §7'nin "fan modu / profil / şarj limiti = active yes"
    # satırı. SSH gibi uzak ya da arka plandaki oturumlar kuraldan düşer ve
    # systemd'nin varsayılanına (yönetici kimlik doğrulaması) kalır. `auth_admin` isteyen
    # DIKKAT sınıfı kontroller henüz sunulmuyor; geldiklerinde AYRI birimler ve
    # AYRI bir kural olacak, bu kurala eklenmeyecek.
    security.polkit.extraConfig = ''
      polkit.addRule(function(action, subject) {
        if (action.id != "org.freedesktop.systemd1.manage-units") return polkit.Result.NOT_HANDLED;
        if (subject.user != "${cfg.user}") return polkit.Result.NOT_HANDLED;
        if (!subject.local || !subject.active) return polkit.Result.NOT_HANDLED;
        var unit = action.lookup("unit");
        if (unit == "aero-fan-cycle.service") return polkit.Result.YES;
        if (unit && unit.indexOf("aero-set-fan@") === 0) return polkit.Result.YES;
        if (unit && unit.indexOf("aero-set-charge@") === 0) return polkit.Result.YES;
        return polkit.Result.NOT_HANDLED;
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
