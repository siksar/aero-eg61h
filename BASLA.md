# Yeni oturum başlatıcısı

> Aşağıdaki bloğu yeni oturuma olduğu gibi yapıştır.

---

`~/aero-eg61h/PLAN.md` dosyasını oku ve uygulamaya devam et.

**Nerede kaldık (7 Eyl 2026):** kararlar kapandı, **adım 1-4 yazıldı ve
doğrulandı** — WMI iskeleti, hwmon, şarj limiti, fan modu. Sürücü artık
`~/nixos-zixar`'ın `aorus_laptop`'tan kullandığı **her şeyi** karşılıyor.
Sıradaki iş **adım 5: `platform_profile`** (yalnız `0xED`, K1 = a′).

**Kullanıcı kararı (7 Eyl): `aorus_laptop` kalıcı olarak bırakılacak.**
Geçiş planı yazıldı ama UYGULANMADI: `docs/nixos-gecis.md` (beş dosya).
`~/nixos-zixar` değişikliği ayrı ve onaylı adım.

Bağlam belgeleri (gerektikçe aç):

- `~/aero-eg61h/PLAN.md` — kapsam, katman mimarisi, panel yapısı, basit/gelişmiş
  ayrımı, tema kaynağı, yetki modeli, boşta güç kısıtı, uygulama sırası (§10),
  istek kuyruğu (§11), kararlar (§12)
- `~/aero-eg61h/kernel/README.md` — **adım 1-4'ün doğrulama çıktıları** ve çekirdek
  API notu (bu çekirdekte `wmi_find_device_by_guid()` YOK)
- `~/aero-eg61h/docs/nixos-gecis.md` — **aorus_laptop → aero_eg61h geçiş planı**
- `~/ecscope/docs/surucu-tasarim.md` — sürücünün ABI kararı; §6 sıra, §7 kararlar
- `~/ecscope/docs/firmware-8051.md` — EC firmware statik analizi + §11 canlı ölçümler
- `~/ecscope/docs/yazma-ve-ic-uzay.md` — 6 Eyl yazma ölçümleri
- `~/ecscope/docs/aero-x16-catalogue.md` — DSDT `WMBD`/`WMBC`/`_Qxx` kataloğu
- `~/ecscope/lib/selectors.tsv` — seçici risk sınıfları
- `~/ecscope/fw/README.md` — ölçüm betikleri ve ortam kurulumu

## Kurallar

- **`~/nixos-zixar`'a habersiz dokunma.** NixOS entegrasyonu ayrı ve onaylı adım.
- **`~/.config/cosmic`'e asla yazma** — oradan yalnız oku.
- **Boşta güç bütçesi 4.28 W gerilemez** — daemon boot'ta başlamaz, yoklama yapmaz.
- **Ölçülmemiş hiçbir yeteneği arayüzde gösterme.** Çalışmayan düğüm koymak,
  düğüm koymamaktan kötüdür (`aorus_laptop`'ın hatası). Bu kural bir modül
  parametresi için bile geçerli: `raw_window` henüz **yok**, çünkü henüz hiçbir
  şey yapmıyor.
- **Kör EC komut denemesi yasak** — `ERCD 0xB0`/`0xB1`'in işleyicisi hâlâ
  bulunamadı, "flash silen komut var mı" sorusu cevapsız.
- **Büyük ajan fan-out'u kullanma** — bu hesapta oturum limitine takılıp sıfır
  sonuç dönüyor (iki kez ölçüldü). Mekanik işi betiğe, yorumu tek bağlama bırak.

## Sıradaki iş — adım 5 (`platform_profile`)

K1 = (a′): handler **yalnız** performans profilini (`WMBD 0xED` 0-3) anahtarlar.
Fan modu pakete GİRMEZ — o zaten kendi sysfs'inde (adım 4).

Kritik kısıt: seçim kümesi `amd-pmf`'inkini (`low-power balanced performance`)
**kapsamalı**. Kapsamazsa `/sys/firmware/acpi/platform_profile` seçenekleri
kesişime düşer, `low-power` kaybolur ve `power-display.nix`'in pildeki
`power-saver` otomatiği — dolayısıyla 4.28 W boşta bütçesi — bozulur.

Doğrulama ÖLÇÜMLE: profiller arası geçiş + `powerprofilesctl` + fiş takıp
çıkarma; `amd-pmf` ile çift yazımın yan etkisi gözlenmeli.

### Paralel duran iki iş

1. **Uyanış kancası testi.** Sürücünün `.resume`'u gerçek bir suspend ile
   denenmedi. Test: `echo 60 > /sys/class/power_supply/BAT1/charge_control_end_threshold`,
   uyut, uyandır, `sudo dmesg | grep uyanis`. Satır yoksa yol
   `register_pm_notifier(PM_POST_SUSPEND)` olacak.
2. **NixOS geçişi.** `docs/nixos-gecis.md`. En kritik maddesi §2: `aorus`
   `fan_mode` sayıları `PECM+0x2C` desenlerine eşlenmiyor, birebir çeviri
   davranışı DEĞİŞTİRİR. AC/BAT varsayılanı `balanced` olmalı.

## Derleme ve yükleme

```bash
cd ~/aero-eg61h/kernel && ./build.sh     # arac zincirini flake'ten ceker

sudo rmmod aorus_laptop                  # SART: ikisi ayni WMI metotlarini cagiriyor
sudo insmod ./aero-eg61h.ko
sudo dmesg | tail                       # SUDO sart: kernel.dmesg_restrict=1

sudo rmmod aero_eg61h && sudo modprobe aorus_laptop   # geri al
```

Sürücü `aorus_laptop` yüklüyse zaten `-EBUSY` ile reddediyor ve dmesg'e
çözümü yazıyor. `force=1` iki kapıyı da atlar — yalnız hata ayıklama için.

## Ölçüm ortamı (gerekirse)

```bash
export SUDO_ASKPASS=$(ls /nix/store/*ksshaskpass*/bin/ksshaskpass | head -1)
sudo -A -E ~/ecscope/fw/fanstate.sh /var/tmp/aero-fw/state-X
python3 ~/ecscope/fw/fancmp.py /var/tmp/aero-fw/state-X
```

Sudo bileti 5 dakikada doluyor. Tazeleyiciyi **kullanıcı** çalıştırmalı:

```
setsid nohup bash -c 'for i in $(seq 1 240); do sudo -n -v || exit 0; sleep 60; done' >/dev/null 2>&1 & disown
```

`nix build` için **`--out-link` kullan**, `--no-link` GC kökü bırakmıyor ve
`nh`'ın GC'si araçları siliyor. `build.sh` bunu `.gcroots/` altında yapıyor.
Ortamda `make`/`python3`/`jq` **yok** — `build.sh` bunları flake'in kendi
pkgs'inden çekiyor (`nix run nixpkgs#…` DEĞİL: o registry'yi çözer).
