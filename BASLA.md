# Yeni oturum başlatıcısı

> Aşağıdaki bloğu yeni oturuma olduğu gibi yapıştır.

---

`~/aero-eg61h/PLAN.md` dosyasını oku ve uygulamaya devam et.

**Nerede kaldık (7 Eyl 2026):** sürücü kararlarının beşi de (K1-K5) kapandı ve
**adım 1 yazılıp doğrulandı** — `kernel/aero-eg61h.{c,h}`, üç `wmi_driver`,
`WMBC`/`WMBD` sarmalayıcıları, DMI + `aorus_laptop` çakışma kapıları.
Sıradaki iş **adım 2: hwmon**.

Bağlam belgeleri (gerektikçe aç):

- `~/aero-eg61h/PLAN.md` — kapsam, katman mimarisi, panel yapısı, basit/gelişmiş
  ayrımı, tema kaynağı, yetki modeli, boşta güç kısıtı, uygulama sırası (§10),
  istek kuyruğu (§11), kararlar (§12)
- `~/aero-eg61h/kernel/README.md` — **adım 1'in doğrulama çıktısı** ve çekirdek
  API notu (bu çekirdekte `wmi_find_device_by_guid()` YOK)
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

## Sıradaki iş — adım 2 (hwmon)

`devm_hwmon_device_register_with_info()` ile **salt okunur** 2 sıcaklık + 2 fan.
`pwm*` **sunulmayacak** (duty yazmaçları inert, üç bağımsız kanıt).

**Önce cevaplanacak tek soru:** `temp2` (`SKTC`, `WMBC 0xE2`) adım 1 koşusunda
boştayken **0** okudu — `aorus_laptop`'ın `temp2_input`'u da aynı anda 0'dı,
yani okuma doğru, alan boş. **Yük altında da 0 ise kanal sunulmayacak.**
Ölçüm: CPU'yu yükle, `WMBC 0xE2`'yi izle.

## Derleme ve yükleme

```bash
cd ~/aero-eg61h/kernel && ./build.sh     # arac zincirini flake'ten ceker

sudo rmmod aorus_laptop                  # SART: ikisi ayni WMI metotlarini cagiriyor
sudo insmod ./aero-eg61h.ko
dmesg | tail

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
