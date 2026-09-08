# Yeni oturum başlatıcısı

> Aşağıdaki bloğu yeni oturuma olduğu gibi yapıştır.

---

`~/aero-eg61h/PLAN.md` dosyasını oku ve uygulamaya devam et.

## ⚠️ ÖNCE BUNU YAP — makine geçici bir aktivasyonda

`nh os test` ile aktive edilmiş bir sistem koşuyor; **profil/boot sistemi eski**.
Reboot edilirse **yazma köprüsü (fan modu / şarj limiti birimleri) kaybolur.**

```bash
readlink -f /run/current-system    # calisan
readlink -f /run/booted-system     # boot — FARKLIYSA test aktivasyonundasin
```

Kalıcı yapmak için (kullanıcının kararı, sorarak yap):

```bash
sudo nixos-rebuild switch --flake ~/nixos-zixar#nixos    # ya da: nh os switch
```

`flake.lock` `aero-eg61h`'i `18913bf`'e pinliyor ve bu **doğru** — `nix/`
dizinine dokunan son commit o. Sonraki commit'ler (logo, eğri verisi, belgeler)
sistem yapılandırmasını etkilemiyor. `nix flake update aero-eg61h` gereksiz.

---

## Nerede kaldık (8 Eyl 2026)

**`aorus_laptop` bırakıldı.** Yerine tam bir yığın var, hepsi ölçümle
doğrulandı:

| katman | durum |
|---|---|
| `kernel/` — `aero_eg61h.ko`: 3 `wmi_driver`, hwmon, şarj limiti, fan modu, `platform_profile` | ✅ boot'tan geliyor |
| `nix/aero-eg61h.nix` — modül, 5 servis, blacklist, polkit | ✅ `verify-context.sh` geçiyor |
| `nix/nixos-zixar-gecis.patch` — geçiş yaması | ✅ uygulandı |
| `app/aero-sysfs` — okuma + yazma katmanı, **18 test** | ✅ yetkisiz çalışıyor |
| `app/aero-ctl` — durum dökümü + `set` | ✅ |
| `app/aero-control` — libcosmic GUI, 6 panel, logo | ✅ salt okuma + basit menü |

`PLAN.md` §10: adım **1-8 ve 11 kapandı**. Kalan: 9 (gelişmiş menü),
10 (EC paneli — **bloke**, aşağı bak), ve Fan Eğrisi paneli (§4.1).

## ⛔ AÇIK KARAR — K2 (MMIO), artık somut bir sebeple

EC İncelemesi paneli ve canlı eğri doğrulaması **yapılamıyor**. Sebep DSDT'den
doğrulandı (ayrıntı: `kernel/README.md`, "debugfs eğri okuyucusu"):

- Hiçbir `WMBC` seçicisi `EIDR`/`ERCD`'ye ulaşmıyor.
- `EIDR` doğrudan çağrılabilir ama **zaman aşımı görülemiyor**: `ECTE` tüm
  DSDT'de üç yerde geçiyor ve üçü de `ERCD`'nin kendi gövdesinde; dışarı veren
  metot yok. `ERCD` zaman aşımını bildirmiyor — `ESRC` bitse bile `ERN1..8`'i
  aynen döndürüyor, **bayat veri geçerli cevaptan ayırt edilemiyor**.
- `ECTE`'yi görmenin tek yolu `0xFC7E0800 + 0x582` ham okuması → `ioremap` →
  **K2 kararının yeniden açılması**.

K2 7 Eyl'de 🚩 bayrakla kapanmıştı ("kullanıcı tam kavramadığını söyledi").
O zaman bedeli soyuttu; şimdi somut:

| MMIO'suz (bugün) | MMIO ile |
|---|---|
| Eğri paneli **çalışır** (veri firmware imajından) | + "EC'de gerçekten bu mu yüklü" doğrulaması |
| EC İncelemesi paneli **yapılamaz** | + panel mümkün |
| Hücre gerilimi yok | + dört hücre gerilimi |
| `ecpoke`/`ecscope` çalışır | `request_mem_region` çağrılmazsa yine çalışır |

**Fan Eğrisi paneli bunu BEKLEMİYOR** — eğriler zaten çıkarıldı
(`app/aero-sysfs/src/curves.rs`, 10 eğri, 7 test). Önce onu yaz, K2'yi
kullanıcıyla sonra konuş.

## Sıradaki iş

1. **Fan Eğrisi paneli** (`PLAN.md` §4.1) — veri hazır, donanıma dokunmuyor.
   Basamak çizimi, iki fan üst üste, canlı sıcaklık imleci, "beş modu birden"
   karşılaştırma kipi. **İnterpolasyon YAPMA** — `t1`/`t2`'nin ara davranışı
   ölçülmedi ve `curves.rs` bunu bir testle sabitliyor.
2. **Adım 9 — gelişmiş menü**: ayrık kontroller + `DIKKAT` sınıfı için onay
   diyaloğu + geri al. `selectors.tsv`'deki `YASAK` sınıfı **hiç gösterilmez**.
3. **Adım 10 — EC paneli**: K2 kararına bağlı, bekliyor.

### Ölçüm borcu (ikisi de kayıtlı)

- `~/nixos-zixar/system/arch/aerox16/wmi.nix`'teki **16 Ağu fan tablosu
  şüpheli** — `aorus_laptop` numaralandırmasıyla alındı, hangi `0x2C` desenini
  ölçtüğü ADJF geçmişine bağlı ve belirsiz. Bizim isimlerimizle
  (quiet/balanced/responsive/gaming/turbo) **yeniden ölçülmeli**.
- `platform_profile` → `0xED 0/1/2` eşlemesi DSDT'nin **statik**
  çözümlemesinden; canlı güç ölçümü yapılmadı.
- COSMIC'te Süper+M fan kısayolu **yok** (`~/.config/cosmic`'e yazmak yasak) —
  elle eklenmeli, komut `nix/README.md`'de.

## Bağlam belgeleri

- `~/aero-eg61h/PLAN.md` — kapsam, paneller, §10 adım tablosu, §13 **10 ölçüm**
- `~/aero-eg61h/kernel/README.md` — sürücünün doğrulama defteri, ECTE tıkanması,
  kaynak bağımsızlığı
- `~/aero-eg61h/app/README.md` — katmanlar, yetki modeli, logo
- `~/aero-eg61h/nix/README.md` — modül, geçiş yaması, uygulama adımları
- `~/aero-eg61h/docs/nixos-gecis.md` — geçişin gerekçesi, `aorus` sayı tuzağı
- `~/ecscope/docs/surucu-tasarim.md` — ABI kararı, K1-K5
- `~/ecscope/docs/firmware-8051.md` — EC firmware analizi, §4 eğri tabloları
- `~/ecscope/lib/selectors.tsv` — seçici risk sınıfları

## Kurallar

- **`~/nixos-zixar`'a habersiz dokunma.** Değişiklik yaparsan **mutlaka**
  `bash scripts/verify-context.sh` çalıştır — `nixos-rebuild build`'in geçmesi
  YETMEZ. (8 Eyl'de geçiş yaması eval'den geçip `deadnix`'e takıldı.)
- **`~/.config/cosmic`'e asla yazma** — yalnız oku.
- **Boşta güç bütçesi 4.28 W gerilemez.**
- **Ölçülmemiş hiçbir yeteneği sunma.** Çalışmayan düğüm koymak, düğüm
  koymamaktan kötüdür. Bu, hiçbir şey yapmayan bir modül parametresi için de
  geçerli.
- **Kör EC komut denemesi yasak** — `ERCD 0xB0`/`0xB1`'in tüketicisi hâlâ
  bulunamadı.
- **Ajan fan-out'u bu hesapta ÇALIŞMIYOR.** 8 Eyl'de 6 slotluk bir workflow
  denendi: **üçü oturum limitine takıldı**. Daha önce 20 ajanla iki kez aynısı
  olmuştu. Mekanik işi betiğe, yorumu tek bağlama bırak.

## Ortam tuzakları (hepsi 7-8 Eyl'de ölçüldü)

| tuzak | çözüm |
|---|---|
| PATH'teki `cargo` bir **rustup kabuğu**, varsayılan toolchain yok | `app/build.sh` araç zincirini flake'ten çeker |
| `/lib/modules/$(uname -r)/build` **yok**, çekirdek clang/LLVM=1 | `kernel/build.sh` |
| `nix build --no-link` GC kökü bırakmıyor, `nh`'ın GC'si araçları siliyor | **`--out-link` kullan**; `.gcroots/` altında |
| `nix run nixpkgs#…` **registry'yi** çözer, flake'in pinini değil | flake'in kendi `pkgs`'inden al |
| `make`, `python3`, `jq`, `rsvg-convert` PATH'te **yok** | flake'ten `nix build` ile çek, `.gcroots/` |
| `dmesg` root ister (`kernel.dmesg_restrict=1`) | `sudo dmesg` ya da `journalctl -b -k` |
| Ağaç-dışı modül güncellemesi **reboot ister** | `modprobe -r` yetmez |
| **Elle `insmod` bir doğrulama DEĞİL** | Boot yarışını yalnız reboot buldu (BAT1 sürücüden 1 sn sonra geliyor) |
| SVG'nin geçerli XML olması "logo iyi" demek değil | Hedef boyutta **render edip BAK** (`librsvg`) |

## Ölçüm ortamı (gerekirse)

```bash
export SUDO_ASKPASS=$(ls /nix/store/*ksshaskpass*/bin/ksshaskpass | head -1)
sudo -A -E ~/ecscope/fw/fanstate.sh /var/tmp/aero-fw/state-X
```

Sudo bileti 5 dakikada doluyor ve arka planda tazeleyiciyi **Claude
başlatamıyor** — kullanıcı çalıştırmalı:

```
setsid nohup bash -c 'for i in $(seq 1 240); do sudo -n -v || exit 0; sleep 60; done' >/dev/null 2>&1 & disown
```

## Hızlı sağlık kontrolü

```bash
~/aero-eg61h/app/target/release/aero-ctl
```

Hiç "EKSİKLER" satırı çıkmamalı. Çıkarsa ne olduğunu ve ne yapılacağını
kendisi yazar.
