# `aero_eg61h` — çekirdek sürücüsü

Gigabyte AERO X16 1VH (SKU **EG61VH**, BIOS FB0A, EC F00A) için WMI platform
sürücüsü. Tasarım kararları `~/ecscope/docs/surucu-tasarim.md`'de, ölçüm tabanı
`~/ecscope/docs/{yazma-ve-ic-uzay,firmware-8051,aero-x16-catalogue}.md`'de.

## Durum: adım 1 tamamlandı (7 Eyl 2026)

| adım | ne | durum |
|---|---|---|
| 1 | İskelet: üç `wmi_driver`, `WMBC`/`WMBD` sarmalayıcıları | ✅ **doğrulandı** |
| 2 | hwmon — 2 sıcaklık + 2 fan, salt okunur | bekliyor |
| 3 | `charge_control_end_threshold` + uyanış kancası | bekliyor |
| 4 | `platform_profile` (yalnız `0xED`, K1 = a′) | bekliyor |
| 5 | Olay kanalı + `sparse_keymap` doldurma | kısmen (log var, tablo yok) |
| 6 | debugfs: eğri okuyucu, `EIDR` (`ECTE` denetimiyle) | bekliyor |

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

> **Adım 2 için açık soru:** `SKTC` boştayken **0** okuyor. Sabit sıfırsa bu ölü
> bir kanaldır ve `temp2` olarak sunmak kendi kuralımızı çiğner ("ölçülmemiş /
> çalışmayan yeteneği sunma"). Yük altında ölçülmeden `temp2` eklenmeyecek.

## Bu adımda bilerek YOK olanlar

- Hiçbir sysfs düğümü, hwmon kanalı ya da `platform_profile` girişi
- Hiçbir yazma yolu (`WMBD` sarmalayıcısı var ama hiç çağrılmıyor)
- MMIO (K2 = B-ops: varsayılan kapalı; `raw_window` parametresi henüz **yok**,
  çünkü hiçbir şey yapmayan bir düğme koymak bu deponun kuralına aykırı)
- `led_classdev` — `KBLL` ölü yazmaç (7 Eyl ölçümü)
- Yazılabilir `pwm*` — duty yazmaçları inert (üç bağımsız kanıt)

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
