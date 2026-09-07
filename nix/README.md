# `nix/` — NixOS entegrasyonu

## `aero-eg61h.nix` — modül

Sürücüyü derleyip yükler, `aorus-laptop`'ı blacklist'ler, `acpi_call`'ı korur,
üç servisi + ACAD udev kuralını + Süper+M polkit kuralını kurar.

Seçenekler: `fanMode.{ac,battery,game,cycle}` · `chargeLimit` ·
`gpuBoost.{ac,battery}`. Varsayılanlar **bugünkü davranışı korur**
(`fanMode.* = "balanced"`, `chargeLimit = 60`, `gpuBoost.ac = 10`).

## `nixos-zixar-gecis.patch` — hazır yama

`~/nixos-zixar`'a uygulanacak değişikliğin tamamı. **Uygulanmadı**; o depo
kullanıcının ve orada başka bekleyen işler var.

```bash
cd ~/nixos-zixar
git apply --check ~/aero-eg61h/nix/nixos-zixar-gecis.patch   # önce dene
git apply         ~/aero-eg61h/nix/nixos-zixar-gecis.patch
bash scripts/verify-context.sh                               # ← ASIL KAPI
sudo nixos-rebuild switch --flake .#nixos                    # senin kararın
```

> **`verify-context.sh` atlanmaz.** `nixos-rebuild build`'in geçmesi yetmez:
> 8 Eyl 2026'da bu yamanın ilk sürümü eval'den geçti ama `deadnix`'e takıldı —
> yeni `wmi.nix` `{ config, pkgs, inputs, ... }` alıp yalnız `inputs`
> kullanıyordu. Yamayı üretirken `nix eval` çalıştırılmış, deponun kendi
> kapısı çalıştırılmamıştı. Düzeltildi (`{ inputs, ... }`), ve ders burada:
> **bu depoda doğrulama `verify-context.sh`'tir**, eval değil.

### Yama ne yapıyor (11 dosya)

| dosya | değişiklik |
|---|---|
| `flake.nix` | `aero-eg61h` girdisi (`git+file://`, `flake = false`) |
| `flake.lock` | girdinin pinlenmiş rev'i |
| `system/arch/aerox16/wmi.nix` | 229 → 110 satır: `aorus-laptop` türetmesi ve üç servis gitti, yerine modül import'u + seçenekler. Fn tuşu hwdb kuralı ve ölçüm defteri KALDI |
| `system/kernel/sched.nix` | `game-perf` turbo: `echo 5 > aorus.../fan_mode` → `echo turbo > .../ABBC0F75-*/fan_mode` |
| `home/desktop/wm/binds.lua` | Süper+M → `aero-fan-cycle.service` |
| `home/desktop/serpantinum/default.nix` | aynı kısayol |
| `home/desktop/CLAUDE.md` | servis adı atıfları |
| `MAINTAINERS` | sürücü değişimi + `fan_mode` eşleme tuzağı + 16 Ağu tablosuna şüphe notu |
| `scripts/{tclt-probe,idle-baseline,diag-game}.sh` | `fan_mode` okuma yolları |

### Doğrulama (7 Eyl 2026)

Yama `~/nixos-zixar`'a **dokunulmadan** üretildi: ağacın izlenen dosyaları geçici
bir dizine kopyalandı, değişiklik orada yapıldı, orada değerlendirildi.

| ne | sonuç |
|---|---|
| Tam sistem evali | ✅ `nixos-system-nixos-26.11...drv` üretildi |
| `boot.blacklistedKernelModules` | ✅ `aorus-laptop` eklendi |
| `boot.kernelModules` | ✅ `aero-eg61h` + `acpi_call`, `aorus-laptop` yok |
| `boot.extraModulePackages` | ✅ `aero-eg61h`, `aorus-laptop` yok |
| Yeni servisler | ✅ `aero-{power-profile,charge-limit,fan-cycle}` |
| Eski servisler | ✅ **hiçbiri kalmadı** (`gigabyte-*`, `fan-mode-cycle`) |
| `git apply --check` gerçek depoda | ✅ temiz uygulanır |
| **`scripts/verify-context.sh`** (uygulandıktan sonra) | ✅ **TÜM KONTROLLER GEÇTİ** — eval, statix, deadnix |

### Uygulamadan sonra

`flake.lock` `aero-eg61h`'i rev `85acce0`'a pinliyor. Sürücüde yeni commit
olursa:

```bash
nix flake update aero-eg61h
```

### Yamaya GİRMEYEN, bilerek

- **`modules/hardware/gaming.nix`** — `sched.nix`'in ikizi gibi duruyor ve aynı
  `aorus_laptop` yolunu taşıyor, ama **hiçbir yerden import edilmiyor**
  (7 Eyl'de doğrulandı; `configuration.nix` yalnız `system/kernel/sched.nix`'i
  alıyor). Eski ağaç düzeninden kalma ölü dosya. Düzenlemek onu bakımlı
  gösterirdi — ya silinmeli ya da başına "import edilmiyor" notu düşülmeli.
  Bu senin kararın.
- **COSMIC kısayolu** — Süper+M `binds.lua` (Hyprland) ve Serpantinum'da bağlı,
  COSMIC'te fan kısayolu hiç yok. `~/.config/cosmic`'e yazmak yasak (kural), o
  yüzden istersen COSMIC Ayarlar'dan elle eklemelisin:
  `systemctl start --no-block aero-fan-cycle.service`
