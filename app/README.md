# `app/` — kullanıcı alanı

Cargo workspace. **Katman katman, her katman tek başına koşturulabilir.**

| crate | ne | bağımlılık | yetki |
|---|---|---|---|
| `aero-sysfs` | veri katmanı — sürücünün sysfs yüzeyini okur | **yok** (std) | **yok** |
| `aero-ctl` | durum dökümü (tanılama + `aero-sysfs`'in testi) | `aero-sysfs` | **yok** |
| `aero-control` | GUI (libcosmic) | *sonra* | okuma yok; yazma ayrı |

## Neden yetki gerekmiyor

Sürücünün sunduğu dört değerin dördü de **dünyaya-okunur** (ölçüldü, 7 Eyl 2026):

```
/sys/class/hwmon/hwmonN/temp1_input                       r--r--r--
/sys/class/power_supply/BAT1/charge_control_end_threshold rw-r--r--
/sys/bus/wmi/devices/ABBC0F75-…/fan_mode                  rw-r--r--
/sys/firmware/acpi/platform_profile                       rw-r--r--
```

Yani salt-okunur GUI **root olmadan** çalışır. Yazma yolu ayrı bir katman
olacak; yetki köprüsü kararı (D-Bus daemon mu, polkit + `systemctl` mi)
henüz verilmedi — bkz. `PLAN.md` §10 adım 6.

## Tek kural: HER DÜĞÜM OPSİYONEL

`aero-sysfs` hiçbir durumda hata döndürmez ve panik etmez. Okunamayan her şey
`None` olur ve sebebi `Snapshot::problems` içinde **insanca** yazılır:

```
$ aero-ctl --root /bos/bir/dizin
  ⚠ EKSİKLER
    Sıcaklık ve fan devri — `aero_eg61h` hwmon cihazı yok — sürücü yüklü değil.
      Yüklemek için: sudo rmmod aorus_laptop && sudo insmod …/aero-eg61h.ko
    Fan modu — WMBD cihazı (ABBC0F75-…) sysfs'te yok — sürücü WMI bus'a bağlanmamış.
    …
```

Bu, **iki iş kolunun buluştuğu sözleşme**: GUI, NixOS geçişini beklemeden
geliştirilebilir ve geçiş yapılmamış bir makinede de anlamlı bir şey gösterir.
`--root` sahte bir sysfs ağacına karşı çalıştırır, yani bu yol makineye
dokunmadan sınanabiliyor — ve sınanıyor (8 test).

## Derleme

```bash
./build.sh          # arac zincirini ~/nixos-zixar flake'inden ceker, test + release
```

PATH'teki `cargo` bir rustup kabuğu ve varsayılan toolchain yok; `build.sh`
araç zincirini flake'ten alır (`kernel/build.sh` ile aynı desen).

## Canlı çıktı (7 Eyl 2026, yetkisiz kullanıcı)

```
AERO X16 1VH (EG61VH) — durum

  Sürücü      : aero_eg61h yüklü
  Güç kaynağı : AC

  CPU         : 51 °C
  Fan 1       : 0 rpm
  Fan 2       : 0 rpm

  Fan modu    : Dengeli (balanced) — 54 °C'de başlar, tavan %43
    seçenekler: quiet balanced responsive gaming turbo

  Profil      : balanced
    seçenekler: low-power balanced performance

  Pil         : %58
  Şarj limiti : %60
```

`Fan 1 = 0 rpm` **doğru**, hata değil: mod 4 eşiğin altında fanı durduruyor.
