# `aero_eg61h` — çekirdek sürücüsü

Gigabyte AERO X16 1VH (SKU **EG61VH**, BIOS FB0A, EC F00A) için WMI platform
sürücüsü. Tasarım kararları `~/ecscope/docs/surucu-tasarim.md`'de, ölçüm tabanı
`~/ecscope/docs/{yazma-ve-ic-uzay,firmware-8051,aero-x16-catalogue}.md`'de.

## Durum: adım 4'e kadar tamam (7 Eyl 2026)

| adım | ne | durum |
|---|---|---|
| 1 | İskelet: üç `wmi_driver`, `WMBC`/`WMBD` sarmalayıcıları | ✅ **doğrulandı** |
| 2 | hwmon — salt okunur sıcaklık + fan | ✅ **doğrulandı** |
| 3 | `charge_control_end_threshold` (+ uyanış kancası) | ✅ **doğrulandı** ¹ |
| 4 | Fan modu — beş mod, özel sysfs | ✅ **doğrulandı** |
| 5 | `platform_profile` (yalnız `0xED`, K1 = a′) | sırada |
| 6 | Olay kanalı + `sparse_keymap` doldurma | kısmen (log var, tablo yok) |
| 7 | debugfs: eğri okuyucu, `EIDR` (`ECTE` denetimiyle) | bekliyor |

¹ Yazma yolu doğrulandı; **uyanış kancası gerçek bir suspend ile HENÜZ test
edilmedi** — ayrıntı aşağıda.

### Sunulan arayüz

```
/sys/class/hwmon/hwmonN/                       (name = aero_eg61h)
    temp1_input  temp1_label                   CPU sicakligi        SALT OKUNUR
    fan1_input   fan1_label                    fan 0 devri          SALT OKUNUR
    fan2_input   fan2_label                    fan 1 devri          SALT OKUNUR

/sys/class/power_supply/BAT1/
    charge_control_end_threshold               sarj limiti %        OKU + YAZ

/sys/bus/wmi/devices/ABBC0F75-8EA1-11D1-00A0-C90629100000-2/
    fan_mode                                   bes mod              OKU + YAZ
    fan_mode_choices                           quiet balanced responsive gaming turbo
```

Hepsi bu. Ölçülmemiş hiçbir şey yok.

### Dosyalar

```
aero-eg61h.h    ortak tanimlar: WMI GUID'leri, secici sabitleri, struct aero_ec
aero-main.c     WMI baglanma, WMBC/WMBD sarmalayicilari, DMI + cakisma kapilari
aero-hwmon.c    hwmon — TAMAMI SALT OKUNUR
aero-battery.c  sarj limiti — standart power_supply ABI'si + uyanis kancasi
aero-fan.c      fan modu — bes desen, dort secicilik yazma dizisi
Makefile        aero-eg61h-y := aero-main.o aero-hwmon.o aero-battery.o aero-fan.o
build.sh        arac zincirini ~/nixos-zixar flake'inden ceker
```

## Derleme

```bash
./build.sh          # arac zincirini ~/nixos-zixar flake'inden ceker
```

`make` doğrudan **çalışmaz**: NixOS'ta `/lib/modules/$(uname -r)/build` yok, ve
CachyOS-lto çekirdeği clang-21 + `LLVM=1` ile derlendi — gcc, kernel
Makefile'ının verdiği `-mstack-alignment=8` / `-fsplit-lto-unit` gibi bayrakları
tanımıyor ve derleme çöker. `build.sh` `kernelModuleMakeFlags`'ı flake'ten alır.

`nix run nixpkgs#…` kullanılmıyor — o, registry'nin nixpkgs'ini çözer, bu
flake'in pinlediğini değil. `nix build` çağrıları `--out-link` ile GC kökü
bırakır (`.gcroots/`), yoksa `nh`'ın GC'si araçları siliyor.

## Yükleme

**`aorus_laptop` önce kaldırılmalı.** İkisi aynı WMI metotlarını çağırıyor;
özellikle fan modu yazımı dört ayrı `WMBD` çağrısından oluşan bir dizi olduğu
için yarış üretir. Sürücü bunu tespit edip bağlanmayı **reddediyor** (`-EBUSY`);
modül yüklenir ama hiçbir cihaza bağlanmaz ve dmesg'de sebebi yazar.

```bash
sudo rmmod aorus_laptop
sudo insmod ./aero-eg61h.ko

# geri:
sudo rmmod aero_eg61h && sudo modprobe aorus_laptop
```

`force=1` hem DMI kapısını hem çakışma kapısını atlar — **yalnız hata ayıklama
için**; başka bir makinede seçici anlamları doğrulanmamıştır.

## Adım 1 doğrulama koşusu (7 Eyl 2026)

```
/sys/bus/wmi/drivers/aero_eg61h       -> ABBC0F75-…-2   (WMBD, yazma)
/sys/bus/wmi/drivers/aero_eg61h_wmbc  -> ABBC0F6F-…-1   (WMBC, okuma)
/sys/bus/wmi/drivers/aero_eg61h_evt   -> ABBC0F72-…-3   (olay, notify 0xD2)

aero_eg61h: EC okuma yolu calisiyor: CPU 37 C, soket 0 C, fan 0/0 rpm
aero_eg61h: fan modu: 0x09 (mod4)
```

Aynı anda `aorus_laptop` **`fan_mode = 1`** diyordu → yanlış bildirim canlı
olarak ikinci kez yakalandı (ilki 7 Eyl öğleden sonra, `ecpoke` ile).

Çapraz doğrulama (`aorus_laptop` hwmon'u ile, saniyeler arayla):

| kanal | biz | `aorus_laptop` |
|---|---|---|
| CPU sıcaklığı | 37 °C | 38 °C (`temp1`) |
| Soket (`SKTC`) | 0 °C | 0 °C (`temp2`) |
| Fan 1 / Fan 2 | 0 / 0 rpm | 0 / 0 rpm |

> **Bu soru adım 2'de kapandı:** `SKTC` yük altında da 0 okuyor → ölü kanal,
> `temp2` sunulmuyor. Ayrıntı aşağıda.

## Adım 2 doğrulama koşusu (7 Eyl 2026)

```
aero_eg61h: EC okuma yolu calisiyor: CPU 45 C, fan 0/0 rpm
aero_eg61h: fan modu: 0x09 (mod4)
aero_eg61h: hwmon hazir: temp1 (CPU), fan1, fan2 — salt okunur
```

`/sys/class/hwmon/hwmonN/` (`name = aero_eg61h`) altında **tam olarak yedi**
dosya: `name`, `temp1_input`, `temp1_label`, `fan1_input`, `fan1_label`,
`fan2_input`, `fan2_label`. Yazılabilir tek bir öznitelik yok, `pwm*` yok.

Yük altında (16 iş parçacığı, `taskset -c 0-15 yes`):

| | boşta | t=15s | t=30s | t=45s | t=60s | t=75s | soğurken |
|---|---|---|---|---|---|---|---|
| `temp1` °C | 45 | 85 | 87 | 89 | 90 | 91 | 56 |
| `fan1` rpm | 0 | 2678 | 3409 | 3409 | 3448 | 3333 | 2127 |
| `fan2` rpm | 0 | 2586 | 3614 | 3658 | 3614 | 3703 | 2702 |

Üç kanal da gerçek fiziği takip ediyor: fanlar mod 4'ün eşiğinde gerçekten
duruyor, yük gelince dönüyor, yük kalkınca düşüyor.

### `SKTC` kapandı — ÖLÜ KANAL

`temp2` adayı olan `SKTC` (`WMBC 0xE2`/`0xE3`, `ECMM+0xB4`) **iki bağımsız
koşuda da 0 okudu**:

- boşta `0` — ve `aorus_laptop`'ın `temp2_input`'u da aynı anda `0`
- **tam yükte, CPU 91 °C ve fanlar 3333/3703 rpm dönerken, hâlâ `0`**

Okuma doğru, alan boş. **`temp2` sunulmayacak** — bu kanal bir daha açılmayacak.

Aynı koşu `aorus_laptop`'ın üçüncü hatasını da belgeliyor: o sürücü bu makinede
`temp2` **ve** `temp3` sunuyor, ikisi de sabit sıfır. `pwm1`/`pwm2` (yazılıyor,
fan umursamıyor) ve `fan_mode` (yanlış değer bildiriyor) ile birlikte, aynı
sürücünün aynı hata sınıfından **üç ayrı örneği**.

### Fan etiketleri: `Fan 1` / `Fan 2`, `CPU Fan` / `GPU Fan` değil

Tasarım belgesi `CPU Fan`/`GPU Fan` öneriyordu — ama bu **ölçülmemiş bir
tahmin**. Firmware fanları yalnız "fan 0" / "fan 1" diye adlandırıyor
(`firmware-8051.md` §4, eğri tabloları ve kural tablosu), DSDT'de de `RPM1`/
`RPM2`'den başka bir isim yok. Hangi fanın neyi soğuttuğu ölçülene kadar
etiketler dürüst kalıyor.

## Adım 3 doğrulama koşusu — şarj limiti (7 Eyl 2026)

```
aero_eg61h: sarj limiti hazir: BAT1/charge_control_end_threshold = 60%
```

`/sys/class/power_supply/BAT1/extensions/` altında `aero_eg61h` göründü
(uzantı kaydı çalışıyor). Yazma testi, **gerçek değişikliklerle**:

| yazılan | geri okunan | hüküm |
|---|---|---|
| 80 | 80 | ✅ |
| 45 | 45 | ✅ |
| 60 (geri alma) | 60 | ✅ |
| 0 | — | reddedildi (`-EINVAL`) |
| 101 | — | reddedildi |
| 255 | — | reddedildi |

Yazımlar sırasında dmesg **sessiz** — yani "yaz + geri oku + karşılaştır"
denetimi hiç uyuşmazlık görmedi. Değer başladığı yerde (%60) bırakıldı.

> **AÇIK: uyanış kancası test edilmedi.** `.resume` geri çağrısı yazıldı ama
> gerçek bir suspend/resume ile denenmedi. PM çekirdeği sürücünün `pm` ops'unu
> yalnız bus kendi bir geri çağrı sunmadığında çağırıyor; WMI bus'ın pm ops'u
> bu çekirdekte incelenemedi (dev çıktısında `drivers/` yok).
> **Test:** limiti yaz → uyut → uyandır → `sudo dmesg | grep uyanis`.
> Satır yoksa yol `register_pm_notifier(PM_POST_SUSPEND)` olacak — o bus'tan
> bağımsız çalışır.

## Adım 4 doğrulama koşusu — fan modu (7 Eyl 2026)

```
fan_mode         = balanced
fan_mode_choices = quiet balanced responsive gaming turbo
```

Beş modun beşi de yazıldı ve **dört seçiciyle geri okunarak** doğrulandı.
`bogus`, `5`, boş dize reddedildi.

**Uçtan uca kanıt — turbo.** Turbo'nun eğrisi düz %63, yani boşta bile fanları
döndürmesi gerekiyor. Boşta 37 °C'de:

| | başlangıç | +2 s | +4 s | `balanced`'a dönüş, +9 s |
|---|---|---|---|---|
| fan 1 | 0 rpm | 4615 | **6976** | 0 |
| fan 2 | 0 rpm | 4838 | **7317** | 0 |

Zincirin tamamı çalışıyor: sysfs yazımı → dört `WMBD` seçicisi →
`PECM+0x2C = 0x0C` → EC turbo eğrisini yüklüyor → fanlar dönüyor →
kendi hwmon'umuz okuyor. Makine `balanced`'da bırakıldı.

### Neden dört seçici ve neden bu sırayla

`PECM+0x2C` bağımsız bir bit alanı değil, **desen eşleştirici**: 16
kombinasyonun yalnız beşi tanınıyor, kalan on biri varsayılana düşüyor.
Deseni yazan tek bir seçici yok — dört ayrı `WMBD` seçicisi dört ayrı biti
yazıyor, ve ara durumlar geçici olarak başka bir moda düşürüyor. Bu yüzden
dizinin tamamı tek kilit altında, ve **önce temizlenecek bitler** yazılıyor:
eksik bitli ara desenler varsayılana (mod 0 — 40 °C'de başlar) düşüyor, yani
ara durum her zaman *daha soğuk* bir moda denk geliyor.

> **`aorus_laptop` yanlış mod bildiriyor — MEKANİZMA ÖLÇÜLDÜ (7 Eyl 2026).**
> Deney: `aorus_laptop` yüklüyken `fan_mode`'a 1/2/5/4 yazıldı, her yazımdan
> önce ve sonra `PECM+0x2C`'nin dört biti `WMBC 0x57/0x71/0x67/0x6A` ile okundu.
>
> | yazılan | `0x2C` öncesi → sonrası | ne yaptı |
> |---|---|---|
> | `1` | `0x09` → `0x09` | b0 kurdu, **b3'e dokunmadı** |
> | `2` | `0x09` → `0x0a` | b1 kurdu, b0'ı temizledi, b3'e dokunmadı |
> | `5` | `0x0a` → `0x0c` | b2 kurdu, b1'i temizledi, b3'e dokunmadı |
> | `4` | `0x0c` → `0x04` | **b3'ü temizledi**, b2'yi bıraktı |
>
> Yani b0/b1/b2'yi birbirini dışlayan bir grup olarak yönetiyor ama **b3'ü
> (`ADJF`) desenin parçası saymıyor**; `fan_mode = 4` bir mod değil, "ADJF'yi
> kapat" işlemi. Ortaya çıkan desen ADJF'nin o anki durumuna bağlı:
>
> | yazılan | ADJF=1 iken | ADJF=0 iken |
> |---|---|---|
> | `1` "sessiz" | `0x09` = mod4 ✗ | `0x01` = quiet ✓ |
> | `2` "gaming" | `0x0a` = **tanınmıyor → varsayılan** ✗ | `0x02` = gaming ✓ |
> | `5` "turbo" | `0x0c` = turbo ✓ | `0x04` = **tanınmıyor → varsayılan** ✗ |
> | `4` "dengeli" | `0x04` = **varsayılan** ✗ | `0x04` = **varsayılan** ✗ |
>
> **Dördünün de doğru çalıştığı bir ADJF durumu yok.** Süper+M döngüsü
> (`4→1→2→5`) ilk adımda ADJF'yi sıfırladığı için ondan sonra hem döngünün
> "Turbo"su hem `game-perf.service`'in `fan_mode = 5`'i `0x04` — yani
> **varsayılan** — veriyor: oyun turbosu sessizce çalışmıyor.
>
> *(Önceki hükmümüz "yalnız `0x57` yazıp kalan üç biti temizlemiyor" idi —
> bu bir çıkarımdı ve mekanizma kısmı YANLIŞTI. Ölçüm düzeltti.)*
> `fan_mode = 1` yazılıyken `balanced` (mod 4) koşuyor.
> Plan: `~/aero-eg61h/docs/nixos-gecis.md` §2.

## Şu ana kadar bilerek YOK olanlar

- `platform_profile` girişi — adım 5 (K1 = a′: yalnız `0xED`)
- `charge_mode` (`WMBD 0x64`, BCPS) — anlamı DSDT'den doğrulanamıyor, standart
  karşılığı yok. Geçiş için `acpi_call` ile kapanıyor (`docs/nixos-gecis.md` §3)
- Fan hızı/duty kaydırıcısı — bu firmware'de fan hızı ayarlanamıyor, yalnız
  hangi eğrinin yükleneceği seçilebiliyor
- Yazılabilir `pwm*` — duty yazmaçları inert (üç bağımsız kanıt)
- Salt okunur `pwm*` bile yok — `FDTY`/`GDTY` gerçek duty'yi göstermiyor
- `temp2` — `SKTC` ölü kanal, yük altında da 0 (yukarı bak)
- `fan3`/`fan4` — donanımda iki fan var, dördüncüsü `aorus_laptop`'ın icadı
- MMIO (K2 = B-ops: varsayılan kapalı; `raw_window` parametresi henüz **yok**,
  çünkü hiçbir şey yapmayan bir düğme koymak bu deponun kuralına aykırı)
- `led_classdev` — `KBLL` ölü yazmaç (7 Eyl ölçümü)

## Çekirdek API notu — tasarım belgesinden sapma

`surucu-tasarim.md` §3.1 "probe'ta `wmi_find_device_by_guid()` + `wmidev_evaluate_method()`"
öneriyordu. **Bu çekirdekte (7.2.2) `wmi_find_device_by_guid()` yok** —
`include/linux/wmi.h`'de bildirilmiyor ve `Module.symvers`'te dışa aktarılmıyor;
GUID ile kardeş cihaz arama API'si kaldırılmış.

Bunun yerine **üç ayrı `wmi_driver`** kaydediliyor ve paylaşılan durum modül
genelinde tutuluyor (`struct aero_ec`). Bağlanma sırası garanti değil, o yüzden
üst katmanlar `WMBD` **ve** `WMBC` ikisi de bağlandığında kuruluyor
(`aero_device_bound()`).

Ayrıca bu çekirdek `notify` yerine `notify_new` sunuyor (`struct wmi_buffer`,
`min_event_size` ile en küçük yük boyutu belirtiliyor) — olay sürücüsü onu
kullanıyor, `min_event_size = 2` (`_WED` 4 bayt döndürüyor, anlamlı kısmı
ilk iki bayt: `[olay_no, durum]`).
