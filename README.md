# `aero-eg61h`

Gigabyte AERO X16 1VH (SKU **EG61VH**, BIOS FB0A, EC F00A) için Linux kontrol
yığını: çekirdek sürücüsü + yetki daemon'u + COSMIC yerlisi bir GUI.

**Durum: adım 1 tamam** (7 Eyl 2026) — çekirdek sürücüsü iskeleti yazıldı,
derlendi ve canlı makinede doğrulandı. Kalan sekiz adım `PLAN.md` §10'da.

## Neden var

Bu makinede üreticinin Linux sürücüsü yok. Elde ya alakasız bir WMI sürücüsü
(`asus-wmi` — yüklü ama hiçbir cihaza bağlı değil) ya da eksik bir topluluk
sürücüsü (`aorus_laptop`) var. İkincisi bu makineye ait olmayan soyutlamalar
dayatıyor ve **çalışmayan kontroller sunuyor** — yazılabilir `pwm` düğümleri
var, fan onları umursamıyor (ölçüldü). Üstelik **yanlış bildiriyor**: adım 1'in
doğrulama koşusunda sürücümüz fan modunu `0x09` (mod 4) okurken `aorus_laptop`
aynı anda `fan_mode = 1` diyordu.

Bu depo, ölçülmüş yer gerçeğinden yazılıyor: neyin çalıştığı `~/ecscope`
takımıyla tek tek ölçüldü, EC firmware'i disassemble edildi.

## Yapı

```
kernel/   aero_eg61h.ko    C, uc wmi_driver, standart ABI'ler   ← adim 1 ✅
daemon/   aero-eg61hd      Rust + zbus, D-Bus + polkit          ← adim 4
gui/      aero-control     Rust + libcosmic                     ← adim 5
dbus/     .conf / .service / polkit kurallari
nix/      flake + NixOS modulu (ayri, onayli adim)              ← adim 9
```

`kernel/README.md` derleme, yükleme ve adım 1'in doğrulama çıktısını taşıyor.

## Yer gerçeği nerede

| belge | ne |
|---|---|
| `~/ecscope/docs/yazma-ve-ic-uzay.md` | canlı yazma ölçümleri, EC iç adres uzayı, tuzaklar |
| `~/ecscope/docs/firmware-8051.md` | EC firmware'inin statik analizi (bankalama, host↔XRAM eşlemesi, fan eğrisi tabloları, komut dağıtımı) |
| `~/ecscope/docs/surucu-tasarim.md` | çekirdek sürücüsünün kapsam/ABI kararı — **K1-K5 kapandı** |
| `~/ecscope/docs/aero-x16-catalogue.md` | DSDT'den çıkarılmış `WMBD`/`WMBC`/`_Qxx` kataloğu |
| `~/ecscope/lib/selectors.tsv` | seçici risk sınıfları (`GUVENLI`/`DIKKAT`/`YASAK`) |

## Kurallar

- **`~/nixos-zixar` altına habersiz dokunulmaz.** NixOS entegrasyonu ayrı ve
  kullanıcı onaylı bir adımdır.
- **`~/.config/cosmic` altına hiçbir şey yazılmaz** — oradan yalnız okunur
  (cosmic-config'in kullanıcı katmanı sistem katmanını yener; store sembolik bağı
  koymak Ayarlar arayüzünün sessizce kaydetmemesine yol açıyor).
- **Boşta güç bütçesi 4.28 W gerilemez.** Daemon boot'ta başlamaz, yoklama yapmaz.
  Sürücü de boştayken hiçbir şey okumaz — tek okuma probe anındaki bağlanma kanıtı.
- **Ölçülmemiş hiçbir yetenek sunulmaz.** Çalışmayan bir düğüm koymak, düğüm
  koymamaktan kötüdür. Bu, hiçbir şey yapmayan bir modül parametresi için de geçerli.
- **Kör EC komut denemesi yasak.** `ERCD 0xB0`/`0xB1`'in işleyicisi firmware'de
  henüz bulunamadı; "flash silen komut var mı" sorusu cevapsız.
