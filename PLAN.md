| 6 | ~~Daemon~~ → **yetki köprüsü** (polkit + PPD, daemon YOK) | `aero-ctl set` yetkisiz çalışıyor, geçersizler reddediliyor | ✅ **8 Eyl 2026** || 8 | **Basit menü** — 5 ön ayar + şarj kaydırıcısı | köprü canlı doğrulandı (yetkisiz); geçersiz girdiler reddediliyor | ✅ **8 Eyl 2026** |# `aero-eg61h` — plan

> 7 Eylül 2026 · Gigabyte AERO X16 1VH (SKU EG61VH) · BIOS FB0A / EC F00A
> Masaüstü: **COSMIC** (System76) · NixOS · çekirdek 7.2.2-cachyos-lto
>
> **Durum: çekirdek katmanı + GUI iskeleti + NixOS geçişi bitti** (§10).
> Kalan: yazma yolu (adım 6 yetki köprüsü, 8-9 menüler) ve EC paneli (10) — sürücü artık fan modunu
> ve şarj limitini yazabiliyor. `aorus_laptop`'a geçiş planı: `docs/nixos-gecis.md`.
> Kararların tamamı §12'de; sürücü kararları (K1-K5) 7 Eyl'de kapandı.

---

## 0. Kullanıcının istediği (kendi sözleriyle)

> "bunları kullanabileceğimiz bir kontrol servisi hazırlamalıyız. uygulamanın sol
> kısmında paneller, basılan panele göre de içeren ayarları yapabildiğimiz kısım
> olması lazım. uygulamanın basit ve gelişmiş menüsü olmalı; basit menüde tavsiye
> edilen ayarlar ile oynanılabilirken gelişmiş menüde metrikleri kullanıcı kendisi
> ayarlayabilecek şekilde olmalı. GUI teması olarak da kişinin kendi tema
> renklerine sahiplenen bir mod, gündüz ve gece modları olmalı. modern dizaynda
> olmalı."

Ek: "onun dışında bir isteğim olursa uygulamayı her deneyimimde söylerim."
→ §11'de bir **istek kuyruğu** var; her yeni istek oraya tarihiyle yazılacak.

---

## 1. Neyi kontrol edeceğiz — yer gerçeği

Tam envanter `~/ecscope/docs/surucu-tasarim.md` ve `firmware-8051.md`'de.
Uygulamanın **sunabileceği** her şey aşağıdaki listeyle sınırlı; bunun dışında
bir düğme koymak yalan olur.

### Gerçekten kontrol edebildiklerimiz (ölçüldü)

| ne | kaynak | değer aralığı |
|---|---|---|
| Fan modu | `WMBD 0x57/0x71/0x67/0x6A` → `PECM+0x2C` | **5 mod**: sessiz(`0x01`) · gaming(`0x02`) · turbo(`0x0C`) · mod0(`0x00`) · **mod4(`0x09`)** |
| Performans profili | `WMBD 0xED` | 0-3 (CPU PL1/2/3 + dGPU bütçesi tek pakette) |
| Şarj limiti | `WMBD 0x65` | 0-100 % — **uyanışta EC geri alıyor, yeniden uygulanmalı** |
| dGPU Dynamic Boost | `WMBD 0x4C` (ACBT), `0x4A` (AMAT), `0xE7` (aç/kapa) | 0-10 / 15-25 / 0-1 |
| CPU termal setpoint | `\DPTT(0x03, N)` (AMD SMU, WMI değil) | °C |

Eğriler **güç kaynağına göre değişmiyor** (7 Eyl ölçümü, `firmware-8051.md` §11.2)
— AC ve pilde aynı tablolar yükleniyor.

### Sadece okuyabildiklerimiz

CPU sıcaklığı, iki fanın RPM'i, AC/pil durumu, kapak, dGPU GC6 kapısı,
**hücre başına pil gerilimi** (Linux'ta bu makinede hiç yok), aktif fan eğrisi
tabloları (yavaş, mailbox üzerinden).

### Asla sunulmayacak

- **Yazılabilir fan hızı/duty** — yazmaçlar ölü (üç bağımsız kanıt).
  `aorus_laptop` bunu sunuyor ve yalan söylüyor; tekrarlamayacağız.
- **Fan eğrisi düzenleyici** — eğri tablosunun veri portu salt-okunur.
- **Soket sıcaklığı (`SKTC`)** — ölü kanal: tam yükte CPU 91 °C iken bile 0
  okuyor (7 Eyl ölçümü). `aorus_laptop` bunu `temp2`/`temp3` olarak sunuyor.
- **Klavye aydınlatma LED sınıfı** — `KBLL` (`WMBD 0xF6`) ölü yazmaç: yazılıyor,
  tutuyor, görsel etkisi yok (7 Eyl ölçümü, aydınlatma açıkken). Aydınlatma
  HID LampArray ile sürülüyor; o ayrı bir iş.
- CMOS'a yazan seçiciler (`0x63 0x87 0x88 0xA3 0xE6`) — kalıcı, geri dönüşü yok.
- `0x51` (dGPU eject), `0xC9` (`FNKS`), `0x4B`, `0xF1/F2/F3` — sistemi düşürebilir
  ya da EC geri yazıyor.

> **Kural:** Gelişmiş menü bile `~/ecscope/lib/selectors.tsv`'deki `YASAK`
> sınıfını **hiç göstermez**. `DIKKAT` sınıfı gösterilir ama onay diyaloğu ister.

---

## 2. Katman mimarisi

Dört katman. Her biri ayrı ayrı test edilebilir ve alttakinin yokluğunda
üsttekinin ne yapacağı tanımlı.

```
┌─────────────────────────────────────────────────────────┐
│ 4. aero-control      GUI (libcosmic) — YETKİSİZ         │
│                      kullanıcı olarak koşar             │
└───────────────────────────┬─────────────────────────────┘
                            │ D-Bus (system bus) + polkit
┌───────────────────────────┴─────────────────────────────┐
│ 3. aero-eg61hd       daemon — root, D-Bus etkinleştirmeli│
│                      boşta çıkar, poll YOK              │
└───────────────────────────┬─────────────────────────────┘
                            │ sysfs / hwmon / power_supply
┌───────────────────────────┴─────────────────────────────┐
│ 2. aero_eg61h.ko     çekirdek sürücüsü (wmi_driver)     │
│                      standart ABI'ler                    │
└───────────────────────────┬─────────────────────────────┘
                            │ ACPI WMBD/WMBC + paylaşımlı pencere
┌───────────────────────────┴─────────────────────────────┐
│ 1. EC firmware       ENE 8051 — bizim değil             │
└─────────────────────────────────────────────────────────┘
```

**Neden daemon var?** GUI root olarak koşmayacak. Yazma işlemleri polkit ile
yetkilendirilecek — `power-profiles-daemon`/`fwupd` ile aynı desen.
Ayrıca şarj limitinin uyanışta yeniden uygulanması gibi **GUI kapalıyken de
gereken** işler var.

**Sürücü yoksa ne olur?** Daemon, sürücü yüklü değilse `acpi_call` üzerinden
ham WMI yoluna düşer (`degraded` kipi) ve GUI bunu üstte bir uyarı şeridiyle
gösterir. Böylece uygulama sürücüden **önce** geliştirilebilir ve test edilebilir.

---

## 3. Teknoloji seçimi

### GUI: **Rust + `libcosmic`** — önerilen

Gerekçe (hepsi kullanıcının isteğine doğrudan cevap veriyor):

| istek | `libcosmic` karşılığı |
|---|---|
| "sol kısımda paneller, basılan panele göre ayarlar" | `cosmic::app::Core` + **`nav_bar`** — COSMIC Settings'in kendisi böyle yapılmış; hazır bileşen |
| "kişinin kendi tema renklerine sahiplenen mod" | `cosmic-theme` kütüphanesi `~/.config/cosmic/com.system76.CosmicTheme.*`'i **canlı** okur; vurgu rengi, yoğunluk, font, ikon teması bedava gelir |
| "gündüz ve gece modları" | `com.system76.CosmicTheme.Mode/v1/is_dark` + `auto_switch` — kütüphane otomatik izler |
| "modern dizayn" | COSMIC'in kendi tasarım dili; sistemle birebir tutarlı |

Ölçülen ortam (7 Eyl): `is_dark = true`, `auto_switch = false`,
`interface_density = Spacious`, font `JetBrainsMono Nerd Font`, ikon teması `macOS`.
`libcosmic` bunların hepsini kendiliğinden uygular.

**Ek kazanç:** Electron/Chromium yok. `~/nixos-zixar/CLAUDE.md`'deki 4.28 W boşta
güç bütçesi ve "ikinci bir Chromium yığını taşıma" dersi (deezer-enhanced'ın
kaldırılma sebebi) bunu zorunlu kılıyor.

**Risk:** `libcosmic` nixpkgs'te ayrı bir paket değil, Cargo git bağımlılığı
olarak tüketiliyor. NixOS'ta paketlemek `rustPlatform.buildRustPackage` +
`cargoLock.outputHashes` gerektirir — COSMIC'in kendi nixpkgs türevleri bunu
zaten yapıyor, desen kopyalanabilir.

**Alternatifler (reddedildi ama kayda geçsin):**
- *GTK4 + libadwaita*: iyi ama COSMIC teması GTK'ya tam yansımıyor; ayrı bir
  tasarım dili olur.
- *Qt6/QML*: Caelestia yığınına uyardı — ama kullanıcı artık COSMIC'te.
- *Tauri/Electron*: modern tasarım kolaylığı var, **boşta güç bütçesini bozar.**

### Daemon: **Rust** (`zbus` ile D-Bus)

GUI ile aynı dil, aynı veri tipleri, tek `cargo workspace`.

### Çekirdek sürücüsü: **C** (`wmi_driver`), ayrı alt dizin

---

## 4. Panel yapısı (sol nav)

Her panel yalnız **gerçekten sunabildiğimiz** şeyleri içerir.

| # | panel | içerik |
|---|---|---|
| 1 | **Durum** | Canlı özet: CPU sıcaklığı, iki fan RPM, güç kaynağı, aktif profil, pil %/sağlık. Salt okunur pano. (Soket sıcaklığı YOK — ölü kanal.) |
| 2 | **Fan ve Termal** | Fan modu seçimi (**5 mod** — mod 4 dahil, aşağı bak). Canlı CPU sıcaklığı + iki fan RPM. *Gelişmiş:* termal setpoint (`DPTT 0x03`). |
| 2b | **Fan Eğrisi** | Seçili modun eğrisini **sıcaklık fonksiyonu olarak** çizen grafik paneli — ayrıntı §4.1 |
| 3 | **Güç ve Performans** | Performans profili (`0xED` 0-3). *Gelişmiş:* dGPU Dynamic Boost bütçesi, AMAT, boost aç/kapa. |
| 4 | **Pil** | Şarj limiti kaydırıcısı. Hücre başına gerilim (4 hücre). Döngü sayısı, sağlık. Uyanışta yeniden uygulama anahtarı. |
| 5 | **EC İncelemesi** *(yalnız Gelişmiş)* | Paylaşımlı pencere görüntüleyici, EC iç uzayı okuyucu, ham `WMBC` seçici okuma. **Salt okunur.** |
| 6 | **Ayarlar** | Basit/Gelişmiş anahtarı, tema kipi, açılışta başlat, telemetri örnekleme aralığı. |

> **"Klavye ve Işıklar" paneli v1'de YOK.** 7 Eyl 2026'da ölçüldü: `KBLL`
> (`WMBD 0xF6`) yazılıyor, tutuyor, geri okunuyor — ama klavyede **hiçbir
> görsel etkisi yok** (aydınlatma açıkken, 0↔4 arası üç kez gidiş-geliş).
> `FDTY`/`FAN1` ile aynı ölü-yazmaç sınıfı. Aydınlatmayı süren yol HID
> LampArray ve o zaten çalışıyor. Klavye özelleştirmesi **ayrı ve sonraki bir
> iş** — LampArray protokolü üzerinden, bu depoda değil.

Panel 6, `~/ecscope`'un yaptığı işi GUI'ye taşır — ama **yalnız okuma**.
Yazma hep `ecpoke`'ta kalır (o ayrımı bozmayacağız).

### 4.1 "Fan Eğrisi" paneli — `duty = f(sıcaklık)`

*(7 Eyl 2026 · kullanıcı isteği)*

Seçili fan modunun eğrisini **sıcaklığın fonksiyonu olarak** çizer. Bu, uygulamanın
en bilgi verici ekranı olacak: fan modunun ne demek olduğunu sayı listesiyle değil
**şekliyle** anlatır.

**Veri.** 14 nokta × `(t1, t2, duty)`. Tabloların hepsi firmware'den çıkarıldı
(24 tablo, `~/ecscope/docs/firmware-8051.md` §4), yani uygulama **hiçbir donanıma
dokunmadan** her modun eğrisini anında çizebilir. Seçim kuralı da biliniyor
(`0x0616E`'deki 8 baytlık kural tablosu), yani "bu modda hangi tablo yüklenir"
sorusu çevrimdışı cevaplanıyor.

| eksen | ne |
|---|---|
| X | sıcaklık (°C) — otomatik aralık, tipik 30-100 |
| Y | duty (%) — 0-100, ama gerçek tavan moda göre 29/43/53/63 |

**Çizim biçimi: basamak (step), eğri değil.** Sebep dürüstlük: `t1`/`t2`'nin
ara değerlerde interpolasyon mu yoksa eşik mi olduğu **henüz ölçülmedi**
(`firmware-8051.md` §9, açık iş 6). Basamak çizmek en az iddia eden gösterimdir.
Ölçüm sonucu interpolasyon çıkarsa çizim yumuşatılır — o zamana kadar
grafiğin altında tek satır: *"eşikler ölçüldü, ara davranış ölçülmedi"*.

**İki eğri, üst üste.** Fan 0 ve fan 1 farklı tablolar kullanıyor (ölçüldü);
ikisi ayrı renkte, efsanede `Fan 1` / `Fan 2`.

**Canlı katman.** Grafiğin üstüne:
- o anki CPU sıcaklığını gösteren **dikey imleç**
- o anki iki fanın RPM'i (grafiğin dışında, sayı olarak)
- imlecin eğriyi kestiği noktada beklenen duty

**Karşılaştırma kipi.** Bir anahtar: "yalnız seçili mod" ↔ "beş modu birden".
İkincisi, mod seçiminin ne değiştirdiğini tek bakışta gösterir — mod 4'ün neden
ilginç olduğu (sessiz gibi geç başlar, varsayılan gibi yükselir) ancak böyle
görülüyor.

**"EC'den doğrula" düğmesi.** Grafik varsayılan olarak firmware kaynak
tablosundan çizilir (anında, yan etkisiz). Düğmeye basılınca aktif eğri
`EIDR` ile EC'den okunur (~1.5 s, 108 bayt) ve kaynak tabloyla karşılaştırılır:
*"EC'deki eğri kaynak tablo 0x05CBE ile birebir aynı ✓"*. Bu, uygulamanın
kullanıcıya **kanıt** sunduğu yer — ve `aorus_laptop`'ın yalan söylediği
durumu (sysfs `1` derken EC'de mod 4 koşması) yakalayan mekanizma.

*Gelişmiş kipte ek olarak:* ham tablo (14 satır × 3 sütun), kaynak ofseti,
kural tablosundaki eşleşen kayıt (`b0`/`b2`/`mod` alanlarıyla).

**Düzenleme YOK.** Eğri salt okunurdur (veri portu salt-okunur, ölçüldü).
Panelde "kaydet" düğmesi olmayacak; bunun yerine grafiğin köşesinde kalıcı bir
not: *"Bu firmware'de eğriler değiştirilemez; mod seçimi hangi eğrinin
yükleneceğini belirler."*

---

## 5. Basit / Gelişmiş ayrımı

Tek bir anahtar (Ayarlar'da ve pencere üst çubuğunda). Panel listesi ve her
panelin içeriği buna göre değişir.

### Basit menü — "tavsiye edilen ayarlarla oynanır"

Ham sayı yok, **isimlendirilmiş hazır kombinasyonlar** var. Her biri fan modu +
performans profilini **tutarlı bir paket** olarak kurar (Gigabyte Control
Center'ın yaptığı gibi):

| ön ayar | `PECM+0x2C` | eğri | `0xED` | ne zaman |
|---|---|---|---|---|
| **Sessiz** | `0x01` | 54 °C'de başlar, tavan %29 | 0 | okuma/yazma, pil ömrü |
| **Dengeli** | `0x09` → **mod 4** | 54 °C'de başlar, tavan %43 | 1 | günlük — *varsayılan* |
| **Duyarlı** | `0x00` → mod 0 | 40 °C'de başlar, tavan %43 | 1 | erken soğutma isteyen |
| **Performans** | `0x02` gaming | 40 °C'de başlar, tavan %53 | 2 | oyun/derleme |
| **Maksimum** | `0x0C` turbo | 36 °C'de başlar, düz %63 | 3 | kısa süreli tam yük |

> **Mod 4, 7 Eyl 2026'da canlı ölçümle keşfedildi** ve makine o sırada zaten
> onda koşuyordu (`0x2C = 0x09`), ama `aorus_laptop` `fan_mode = 1` diyordu.
> Eğrisi "sessiz gibi geç başla, varsayılan gibi yükselebil" — günlük kullanım
> için en dengeli seçenek, ve **hiçbir Linux aracı bunu sunmuyor.**
> Bu yüzden "Dengeli" ön ayarı mod 0'ı değil mod 4'ü kullanıyor.

Artı bir kaydırıcı: **şarj limiti** (tavsiye 60/80/100 işaretli).

Basit menüde toplam 6 kontrol var (5 ön ayar + 1 kaydırıcı). Amacı: bir şeyi
bozamamak.

### Gelişmiş menü — "metrikleri kullanıcı kendisi ayarlar"

- Fan modu ve performans profili **ayrı ayrı** (paket bozulabilir)
- dGPU Dynamic Boost bütçesi (ham 0-10 ve karşılığı watt)
- CPU termal setpoint (`DPTT 0x03`, °C)
- Şarj limiti serbest sayı girişi
- Aktif eğrinin ham tablosu + hangi kaynak tablodan geldiği
- EC İncelemesi paneli açılır
- Her `DIKKAT` sınıfı kontrol için **onay diyaloğu** ("bu ayar EC tarafından geri
  alınabilir / yan etkisi ölçülmedi") ve **geri al** düğmesi

> **Değişmez kural:** Gelişmiş menü, olmayan bir yeteneği sunmaz. Fan hızı
> kaydırıcısı **yok**, çünkü fan hızı ayarlanamıyor. Bunun yerine o alanda
> tek satırlık bir açıklama durur: *"Bu firmware'de fan hızı doğrudan
> ayarlanamıyor; yalnız hangi eğrinin kullanılacağı seçilebiliyor."*

---

## 6. Tema

**Kaynak: COSMIC'in kendi yapılandırması.** Uygulama kendi renk paleti taşımaz.

```
~/.config/cosmic/com.system76.CosmicTheme.Mode/v1/is_dark      → gece/gündüz
~/.config/cosmic/com.system76.CosmicTheme.Mode/v1/auto_switch  → otomatik geçiş
~/.config/cosmic/com.system76.CosmicTheme.Dark/v1/…            → koyu palet
~/.config/cosmic/com.system76.CosmicTheme.Light/v1/…           → açık palet
~/.config/cosmic/com.system76.CosmicTk/v1/…                    → yoğunluk, font, ikon
```

`libcosmic` bunları zaten okuyor ve **canlı izliyor** — kullanıcı COSMIC
Ayarlar'dan vurgu rengini değiştirdiğinde uygulama anında uyar. Bizim yazacağımız
kod: hiç.

Ayarlar panelinde üç seçenek: **Sistemi izle** (varsayılan) · **Açık** · **Koyu**.
"Sistemi izle" `auto_switch`'i de onurlandırır.

> **YASAK:** `~/.config/cosmic` altına **hiçbir şey yazılmayacak.**
> `~/nixos-zixar/CLAUDE.md`'nin kuralı: cosmic-config'in kullanıcı katmanı
> sistem katmanını yener ve oraya bir store sembolik bağı koymak Ayarlar
> arayüzünün **sessizce** kaydetmemesine yol açıyor. Biz oradan sadece **okuruz**.

---

## 7. Yetki modeli

| eylem | kim | nasıl |
|---|---|---|
| Sensör okuma | GUI doğrudan | `hwmon`/`power_supply` sysfs — zaten herkese açık |
| Fan modu / profil değiştirme | daemon | polkit `org.aero.eg61h.set-profile` — `active` oturum için `yes` |
| Şarj limiti | daemon | polkit `…set-charge-limit` |
| Gelişmiş / `DIKKAT` ayarları | daemon | polkit `…advanced` — **her seferinde parola** (`auth_admin`) |
| EC ham okuma | daemon | polkit `…read-ec` — `active` için `yes` (okuma zararsız) |
| EC yazma | **hiç yok** | GUI'de yazma yolu bulunmayacak; ölçüm hep `ecpoke`'ta |

---

## 8. Boşta güç kısıtı — bu depoda pazarlık konusu değil

`~/nixos-zixar/CLAUDE.md`: *"pil/idle tabanı 4.28 W GERİLEMEZ"*, ve
*"`system/` ya da `home/` altına eklenen hiçbir şey boştayken koşmamalı ya da
yoklama yapmamalı."*

Bunun uygulamaya yansıması:

1. **Daemon D-Bus ile etkinleştirilir** (`BusName=`, `Type=dbus`), iş bitince
   `IdleTimeout` ile **çıkar**. Boot'ta başlamaz.
2. **Hiçbir yoklama yok.** Sensörler yalnız GUI açıkken ve yalnız görünür panel
   için okunur. GUI kapanınca örnekleme durur.
3. AC/pil değişimi **udev olayıyla** yakalanır, zamanlayıcıyla değil — güç
   katmanının mevcut deseni budur.
4. Şarj limitinin uyanışta yeniden uygulanması `systemd` `sleep` kancasıyla
   yapılır, sürekli koşan bir servisle değil.
5. Örnekleme aralığı Ayarlar'da (varsayılan 2 sn, en hızlı 1 sn); "Durum"
   paneli dışındaki paneller açıkken örnekleme yavaşlar.

**Kabul ölçütü:** uygulama kurulduktan sonra, GUI kapalıyken
`nix store diff-closures` ve boşta güç ölçümü **değişmemeli**.

---

## 9. Depo düzeni

```
~/aero-eg61h/
├── PLAN.md                 ← bu dosya
├── README.md
├── kernel/                 aero_eg61h.ko  (C) — üç wmi_driver, beş katman
├── app/                    cargo workspace
│   ├── aero-sysfs/         veri katmanı — salt okunur, std-only, YETKİSİZ
│   ├── aero-ctl/           durum dökümü (tanılama + aero-sysfs'in testi)
│   └── aero-control/       GUI (libcosmic)
├── nix/                    NixOS modülü + hazır geçiş yaması
├── scripts/                doğrulama betikleri
└── docs/
```

> **§9 8 Eyl 2026'da güncellendi.** Eski düzen `daemon/` + `gui/` + `dbus/`
> ayırıyordu. `daemon/` ve `dbus/` HENÜZ YOK ve olmayabilir: daemon'un iki
> gerekçesinden biri (GUI kapalıyken şarj limitini uyanışta yeniden uygulamak)
> sürücünün kendi `.resume` kancasına taşındı ve orada doğrulandı. Geriye yalnız
> yetki aracılığı kaldı; onun için D-Bus daemon'u tek seçenek değil — `nix/`
> modülünün zaten kullandığı polkit + `systemctl start` deseni daha hafif ve
> §7'nin eylem-başına yetki tablosuna daha doğrudan oturuyor. Karar adım 6'da,
> ilk yazma yolu GUI'ye geldiğinde verilecek.

**`~/nixos-zixar`'a hiçbir şey eklenmeyecek** — entegrasyon ayrı bir adım ve
kullanıcının kararı (`PROMPT.md` kuralı).

---

## 10. Uygulama sırası

Her adım tek başına çalışır ve tek başına doğrulanır.

| # | adım | doğrulama | durum |
|---|---|---|---|
| 1 | **Çekirdek sürücüsü iskeleti** — üç `wmi_driver`, `WMBC`/`WMBD` sarmalayıcı | `/sys/bus/wmi/drivers/aero_eg61h` görünür | ✅ **7 Eyl 2026** |
| 2 | **hwmon** — 1 sıcaklık + 2 fan, salt okunur (SKTC ölü çıktı) | yük altında CPU 45→91 °C, fanlar 0→3400 rpm | ✅ **7 Eyl 2026** |
| 3 | **`charge_control_end_threshold`** + uyanış kancası | 60→80→45→60 geri okumayla eşleşti; kanca uyanışta tetiklendi | ✅ **7 Eyl 2026** |
| 4 | **Fan modu** — beş mod, özel sysfs (K1 = a′: `platform_profile`'a girmez) | turbo boşta fanları 0→7000 rpm'e çıkardı | ✅ **7 Eyl 2026** |
| 5 | **`platform_profile`** — yalnız `0xED` | legacy seçenekler modülsüz hâlle **birebir aynı**; üç profil iki handler'a birden gidiyor | ✅ **7 Eyl 2026** |
| 6 | **Daemon** — D-Bus arayüzü + polkit, sürücüsüz `degraded` kipi dahil | `busctl` ile elle çağırma | |
| 7 | **GUI iskeleti** — `nav_bar` + 5 panel, hepsi salt okunur | 594 crate derlendi; Wayland oturumunda çöküşsüz açıldı | ✅ **8 Eyl 2026** |
| 8 | **Basit menü** — 5 ön ayar + 1 kaydırıcı | ön ayara basınca `fan_mode` gerçekten değişiyor | |
| 9 | **Gelişmiş menü** — ayrık kontroller + onay diyalogları | `DIKKAT` ayarları parola istiyor, geri al çalışıyor | |
| 10 | **EC İncelemesi paneli** | çıktı `ecpoke` ölçümüyle bayt-birebir | |
| 11 | **NixOS geçişi** — `aorus_laptop` bırakılır | yama uygulandı, `verify-context.sh` geçti, switch edildi | ✅ **8 Eyl 2026** (reboot bekliyor) |

Adım 7'ye kadar sürücü şart değil (daemon `acpi_call` yoluna düşer), yani GUI
paralel geliştirilebilir.

### Adım 1 — ne yapıldı (7 Eyl 2026)

`kernel/` altında `aero_eg61h.ko`: üç `wmi_driver` (`ABBC0F75` yazma,
`ABBC0F6F` okuma, `ABBC0F72` olay), `WMBC`/`WMBD` sarmalayıcıları, DMI kapısı,
`aorus_laptop` çakışma kapısı. **Hiçbir sysfs düğümü ve hiçbir yazma yolu yok.**
Ayrıntı ve doğrulama çıktısı: `kernel/README.md`.

Koşu iki şeyi kanıtladı: okuma yolu çalışıyor (CPU 37 °C, `aorus_laptop`'ın
38 °C'siyle uyumlu) ve **fan modu `0x09` = mod 4** okunuyorken `aorus_laptop`
aynı anda `fan_mode = 1` diyordu — yanlış bildirim canlı olarak ikinci kez
yakalandı.

> **Tasarım belgesinden bir sapma var.** `surucu-tasarim.md` §3.1 kardeş WMI
> cihazını `wmi_find_device_by_guid()` ile bulmayı öneriyordu; **o API bu
> çekirdekte (7.2.2) yok** — ne başlıkta bildiriliyor ne `Module.symvers`'te
> dışa aktarılıyor. Yerine üç ayrı sürücü + modül genelinde paylaşılan durum
> kullanıldı. Gerekçe `kernel/README.md`'nin son bölümünde.

### Adım 2 — ne yapıldı (7 Eyl 2026)

`kernel/aero-hwmon.c`: **tamamı salt okunur** hwmon. Üç kanal —
`temp1` (CPU, `WMBC 0xE1`), `fan1` / `fan2` (`WMBC 0xE4`/`0xE5`) + etiketleri.
`/sys/class/hwmon/hwmonN/` altında tam olarak yedi dosya, **yazılabilir tek bir
öznitelik yok**, `pwm*` yok.

Yük altında doğrulandı (16 iş parçacığı, 75 s): CPU 45 → 91 °C, fanlar
0 → 3400/3700 rpm, yük kalkınca 56 °C / 2127 / 2702. Üç kanal da gerçek fiziği
takip ediyor; fanlar mod 4'ün eşiğinde gerçekten duruyor.

> **`SKTC` kapandı — ölü kanal.** `temp2` adayı iki bağımsız koşuda da 0 okudu:
> boşta, **ve tam yükte CPU 91 °C / fanlar 3333-3703 rpm dönerken**. Okuma
> doğru, alan boş → `temp2` sunulmuyor. Aynı koşu `aorus_laptop`'ın **üçüncü**
> hatasını belgeliyor: o `temp2` ve `temp3` sunuyor, ikisi de sabit sıfır.
>
> **Fan etiketleri `Fan 1`/`Fan 2`.** Tasarım belgesi `CPU Fan`/`GPU Fan`
> diyordu — ölçülmemiş bir tahmin. Firmware fanları yalnız "fan 0"/"fan 1"
> diye adlandırıyor, DSDT'de `RPM1`/`RPM2` dışında isim yok.

---

### Adım 3 ve 4 — ne yapıldı (7 Eyl 2026)

Sürücünün **ilk iki yazma yolu**. İkisi de aynı deseni kullanıyor:
**yaz → geri oku → karşılaştır**. Sebebi ölçülmüş bir gerçek: `WMBD`'nin dönüş
değeri hiçbir bilgi taşımıyor, tanınmayan seçici bile girdiyi yankılıyor. Tek
doğrulama yolu karşılık gelen `WMBC` seçicisiyle geri okumak.

**Adım 3 — `kernel/aero-battery.c`.** `power_supply` uzantısı, `BAT1` üzerine
standart `charge_control_end_threshold`. Özel `charge_limit` düğümü yok.
Uyanış kancası var (EC limiti uyanışta geri alıyor) ama **yalnız biz yazdıysak**
devreye giriyor — kullanıcının dokunmadığı bir ayarı sürücü zorlamaz.
Doğrulama: 60 → 80 → 45 → 60, üçü de geri okumayla eşleşti; 0/101/255 reddedildi.

**Adım 4 — `kernel/aero-fan.c`.** Beş mod, özel sysfs
(`/sys/bus/wmi/devices/ABBC0F75-…-2/fan_mode` + `fan_mode_choices`).
`platform_profile`'a **girmiyor** — K1 = (a′) gereği ayrı kol.

Uçtan uca kanıt turbo'dan geldi: boşta 37 °C'de fanlar 0 → 4615/4838 →
**6976/7317 rpm**, `balanced`'a dönünce 9 s içinde 0. Yani sysfs yazımı →
dört `WMBD` seçicisi → `PECM+0x2C = 0x0C` → EC eğri yükleme → fan → hwmon
zincirinin tamamı çalışıyor.

> **`aorus_laptop` yanlış mod bildiriyor — MEKANİZMA ÖLÇÜLDÜ (7 Eyl 2026).**
> O sürücü `PECM+0x2C`'nin b0/b1/b2'sini birbirini dışlayan bir grup olarak
> yönetiyor ama **b3'ü (`ADJF`) desenin parçası saymıyor**. Sonuç ADJF'nin o
> anki durumuna bağlı ve dördünün de doğru çalıştığı bir durum yok:
> `fan_mode = 1` ADJF=1 iken `0x09` (mod4) veriyor — makinenin aylardır
> "sessiz" yazılıyken `balanced` koşmasının sebebi bu. Daha kötüsü: Süper+M
> döngüsü ADJF'yi sıfırlıyor, ondan sonra `game-perf`'in `fan_mode = 5`'i
> turbo değil **varsayılan** veriyor. Tam tablo ve deney:
> `~/aero-eg61h/docs/nixos-gecis.md` §2.
> ("sessiz") yazılıyken **`balanced` (mod 4)** koşuyor, o yüzden geçiş
> sayıları birebir çeviremez: `docs/nixos-gecis.md` §2.

## 11. İstek kuyruğu

Kullanıcı uygulamayı denedikçe buraya yazılacak. Biçim: tarih · istek · durum.

| tarih | istek | durum |
|---|---|---|
| 2026-09-07 | Sol panel + panele göre içerik | planda (§4) |
| 2026-09-07 | Basit / Gelişmiş menü ayrımı | planda (§5) |
| 2026-09-07 | Kullanıcının kendi tema renklerini devralma + gündüz/gece | planda (§6) |
| 2026-09-07 | Modern tasarım | planda (§3 — libcosmic) |
| 2026-09-07 | Seçilen fan modunu **sıcaklık fonksiyonu** olarak çizen eğri paneli | planda (§4.1) |
| 2026-09-07 | Klavye özelleştirme | **ertelendi** — `KBLL` ölü çıktı; LampArray yolu ayrı iş |

---

## 12. Kararlar

> **İsim çakışması uyarısı.** Bu dosyanın `D` kararları **uygulamaya** ait;
> `~/ecscope/docs/surucu-tasarim.md` §7'nin `K` kararları **sürücüye** ait.
> İkisi ayrı numaralandırma. `BASLA.md` 7 Eyl'de "D1 (platform_profile) /
> D2 (MMIO)" diye yazmıştı — kastedilen K1 ve K2'ydi; ikisi de aşağıda kapalı.

### Uygulama kararları

| # | soru | karar |
|---|---|---|
| D1 | GUI araç seti: `libcosmic` mi? | **Evet** — COSMIC'te olduğunuz sürece en iyi uyum. COSMIC'ten vazgeçilirse uygulama yine çalışır, sadece tema varsayılana düşer. |
| D2 | Uygulama adı | `aero-control` (ikili), görünen ad **"AERO Kontrol"** |
| D3 | Daemon mu, yoksa GUI doğrudan polkit/pkexec mi? | **Daemon** — uyanış kancası ve GUI'siz işler için gerekli |
| D4 | `aorus_laptop` ne olacak? | Geliştirme boyunca elle `rmmod`; kalıcı blacklist sürücü olgunlaşınca ve **sizin onayınızla** |
| D5 | Klavye ışığı panele girsin mi? | **KAPANDI — HAYIR.** 7 Eyl ölçümü: `KBLL` görsel etkisi yok, ölü yazmaç. `led_classdev` sunulmayacak, panel v1'de yok. Klavye özelleştirmesi LampArray ile, ayrı iş. |

### Sürücü kararları (`surucu-tasarim.md` §7) — 7 Eyl 2026'da kapandı

| # | soru | karar |
|---|---|---|
| K1 | `platform_profile` | **(a′) yalnız `0xED` taşısın.** Handler SADECE performans profilini anahtarlar; seçim kümesi `amd-pmf`'inkini (`low-power balanced performance`) **kapsar**, böylece `/sys/firmware/acpi/platform_profile` kesişimi daralmaz ve `power-display.nix`'in pildeki `power-saver` otomatiği bozulmaz. **Fan modu ayrı bir koldur** — yoksa PPD'nin AC/BAT otomatiği her fiş takışında kullanıcının fan seçimini sessizce geri alırdı. |
| K2 | MMIO | **B-ops.** Varsayılan MMIO yok; ham pencere ileride `raw_window=1` modül parametresiyle isteğe bağlı açılır ve o kipte de `request_mem_region` **çağrılmaz**, yani `ecscope`/`ecpoke`'un `/dev/mem` yolu her iki durumda da açık kalır. `CONFIG_IO_STRICT_DEVMEM=y` **ölçüldü** (7 Eyl) — risk teorik değil. 🚩 **BAYRAK:** kullanıcı bu kararı tam kavramadığını söyledi, öneri üzerinden gidildi. Hücre gerilimi paneli gündeme geldiğinde yeniden konuşulacak. |
| K3 | Depo | **`~/aero-eg61h`** (ayrı depo; `~/nixos-zixar`'a habersiz dokunulmaz) |
| K4 | `aorus_laptop` | **(b) KALICI GEÇİŞ — kullanıcı 7 Eyl'de onayladı** ("bundan sonra aorus laptop wmi yerine sadece bunu kullanacagiz"). Özellik eşitliği adım 3+4 ile sağlandı. Uygulama planı `docs/nixos-gecis.md`; `~/nixos-zixar` değişikliği ayrı ve onaylı adım (§10 adım 11). Geliştirme sırasında sürücü çakışmayı kendi tespit edip `-EBUSY` ile reddediyor. |
| K5 | Manuel fan duty izi | **(b) sonraya.** v1 zaten `pwm` sunmuyor; firmware'deki manuel duty yolunun host'tan erişilebilirliği bulunursa v2'de eklenir. |

---

## 13. Ölçümler

### ✅ Ölçüm 1 — AC/DC eğri ayrımı — **YAPILDI 7 Eyl 2026, hipotez ÇÜRÜDÜ**

Kural tablosundaki `b0` alanının (`0x00`/`0x10`/`0x30`/`0x40`) güç kaynağı
ayrımı olduğu sanılıyordu. **Değil.**

Yöntem: önce pilde, sonra fişte fan kayıt dizisi (`0xF8A4`, 108 bayt) okundu.
İlk karşılaştırma sonuçsuz kaldı — **EC eğrileri yalnız mod değişiminde yeniden
yüklüyor**, AC/DC geçişi tetiklemiyor. Bu yüzden AC'deyken mod geçici olarak
`0x01`'e (sessiz) alınıp yeniden yükleme zorlandı, sonra geri yüklendi
(`fw/fanmode-probe.sh`, `trap` ile garantili geri yükleme, doğrulandı).

Sonuç: AC'de sessiz mod `0x05A66`/`0x05AB1` yüklüyor — **pildekiyle birebir aynı.**

**Sürücü/uygulama sonucu:** eğriler güç kaynağına göre değişmiyor.
"Fan Eğrisi" paneli tek bir eğri seti gösterecek, AC/DC ayrımı olmayacak.
`b0 = 0x30`/`0x40` tablolarının ne olduğu **açık soru** olarak kalıyor.

**Yan bulgu (daha değerli):** ölçüm sırasında makinenin **mod 4**'te koştuğu
ortaya çıktı (`PECM+0x2C = 0x09`) — `aorus_laptop` ise `fan_mode = 1` diyordu.
Mod 4 daha önce "erişilemez" sanılıyordu. §5'e işlendi.

**Yan bulgu 2:** hücre başına pil gerilimi kanalı canlı doğrulandı
(3829/3844/3848/3850 mV, toplam `voltage_now`'u %0.3 içinde takip ediyor).

### ✅ Ölçüm 2 — klavye ışığı görsel testi — **YAPILDI 7 Eyl 2026, ÖLÜ YAZMAÇ**

`fw/kbll-test.sh`, aydınlatma **açıkken** iki koşuda çalıştırıldı (0↔4 arası üç
gidiş-geliş, 5 sn bekleme). `KBLL` (`WMBD 0xF6`) yazılanı tutuyor ve `WMBC 0xF6`
geri okuması doğruluyor — ama klavyede **ne parlaklık ne renk değişti**.
Ayrıca klavye yanarken başlangıç değeri **0** okudu.

`FDTY`/`FAN1`/`XFNW` ile aynı **ölü yazmaç** sınıfı. Aydınlatmayı süren yol HID
LampArray ve o zaten çalışıyor.

**Sonuç:** D5 kapandı (HAYIR), `led_classdev` sunulmayacak, "Klavye ve Işıklar"
paneli v1'de yok. Klavye özelleştirmesi LampArray protokolü üzerinden, ayrı ve
sonraki bir iş.

### ✅ Ölçüm 3 — çekirdek durum taraması — **YAPILDI 7 Eyl 2026 (akşam)**

Adım 1'i yazmadan önce sürücünün bağlanacağı zemin ölçüldü:

| ne | ölçüm | sürücüye etkisi |
|---|---|---|
| `/sys/bus/wmi/drivers/` | **boş**, dört GUID sahipsiz | `wmi_driver` yolu açık |
| `platform-profile-0` | sahibi `amd-pmf`, seçenekler `low-power balanced performance` | K1 = (a′): seçim kümemiz bunu **kapsamalı** |
| `power-profiles-daemon` | 0.30, `PlatformDriver: platform_profile` | 4. adımda birlikte ölçülecek |
| `BAT1/extensions/` | **var ve boş** | `charge_control_end_threshold` temiz eklenir |
| `CONFIG_IO_STRICT_DEVMEM` | **`y`** | K2'nin riski gerçek: `request_mem_region` `/dev/mem`'i kapatır |
| `wmi_find_device_by_guid()` | başlıkta **yok**, `Module.symvers`'te **yok** | tasarımdan sapıldı: üç ayrı sürücü |
| DMI | `GIGABYTE` / `EG61VH` / BIOS `FB0A` | DMI kapısı bu ikiliyle eşleşiyor |

### ✅ Ölçüm 4 — `SKTC` yük altında — **YAPILDI 7 Eyl 2026, ÖLÜ KANAL**

`temp2` adayının canlı olup olmadığı adım 2'nin ön koşuluydu. Yöntem: hwmon
kanalları okunurken 16 iş parçacığı yük (`taskset -c 0-15 yes`), 75 s.

| | boşta | t=15s | t=30s | t=45s | t=60s | t=75s | soğurken |
|---|---|---|---|---|---|---|---|
| `temp1` (CTMP) °C | 45 | 85 | 87 | 89 | 90 | 91 | 56 |
| `fan1` rpm | 0 | 2678 | 3409 | 3409 | 3448 | 3333 | 2127 |
| `fan2` rpm | 0 | 2586 | 3614 | 3658 | 3614 | 3703 | 2702 |
| **`SKTC`** (`0xE2`) | **0** | | | | | **0** | |

**Sonuç: `SKTC` ölü.** CPU 91 °C, fanlar 3333/3703 rpm dönerken bile 0.
Okuma yolu doğru (aynı çağrı zinciri `CTMP`'yi doğru okuyor), alan boş.
`temp2` sunulmuyor ve bu kanal bir daha açılmayacak. `0xE3` aynı alanı okuduğu
için o da ölü.

**Yan bulgu:** `aorus_laptop` bu makinede `temp2` **ve** `temp3` sunuyor, ikisi
de sabit sıfır. `pwm1`/`pwm2` (yazılıyor, fan umursamıyor) ve `fan_mode`
(yanlış değer bildiriyor) ile birlikte, aynı sürücünün aynı hata sınıfından
**üç ayrı örneği** — bu deponun var olma sebebinin canlı belgesi.

**Yan bulgu 2:** mod 4'ün eğrisi doğrulandı — 91 °C'de fanlar yalnız ~3400/3700
rpm'de kalıyor (tavan %43). Turbo'nun düz %63'ü ile arasındaki fark bu.

### ✅ Ölçüm 5 — fan modu yazma zinciri — **YAPILDI 7 Eyl 2026**

`PECM+0x2C` desenini dört `WMBD` seçicisiyle yazıp dört `WMBC` seçicisiyle geri
okuma zinciri, beş modun beşinde de doğrulandı. Asıl kanıt turbo'dan geldi,
çünkü tek gözle görülür (ve duyulur) olan o:

| boşta 37 °C | başlangıç | +2 s | +4 s | `balanced`, +9 s |
|---|---|---|---|---|
| fan 1 | 0 rpm | 4615 | **6976** | 0 |
| fan 2 | 0 rpm | 4838 | **7317** | 0 |

Turbo'nun eğrisi düz %63, yani boşta bile dönmesi gerekiyordu — döndü.
`quiet`/`responsive`/`gaming`/`balanced` boştayken fanları çalıştırmadı, bu da
eğrilerinin başlangıç eşiklerine (54/40/40/54 °C) uygun.

### ✅ Ölçüm 6 — `charge_mode` (BCPS) — **YAPILDI 7 Eyl 2026**

`~/nixos-zixar`'ın `gigabyte-charge-limit.service`'i `charge_mode = 1` yazıyor
ve dosyadaki yorum "charge_limit yalnız custom charge_mode'da çalışır" diyor.
Sürücümüz `charge_mode` sunmuyor, o yüzden geçiş öncesi ölçüldü:

| seçici | okunan | yorum |
|---|---|---|
| `WMBC 0x64` (BCPS) | **`0x04`** | `aorus`'un "1"'i EC'de **4** üretiyor — sayı eşlemesi yine birebir değil |
| `WMBC 0x65` (BCPC) | `0x3c` = 60 | bizim sysfs'imizle birebir |
| `WMBC 0xA2` | `0` | ölçüm anında **pilde** |

**Sonuç:** boşluk tahminle değil `acpi_call` ile kapanıyor —
`\_SB.PCI0.AMW0.WMBD 0 0x64 4`. Ayrıntı `docs/nixos-gecis.md` §3.

### ✅ Ölçüm 8 — uyanış kancası — **YAPILDI 7 Eyl 2026, 19:06**

```
[83674.484050] aero_eg61h: uyanis: sarj limiti 60% korunmus, dokunulmadi
```

**Kapanan soru:** `driver->pm` WMI bus'ında tetikleniyor mu? **Evet.**
`wmi_bus_type` kendi bir resume geri çağrısı sunmuyor, dolayısıyla sürücünün
`DEFINE_SIMPLE_DEV_PM_OPS`'u çalışıyor. `register_pm_notifier` yoluna gerek yok.

**Açılan soru:** kancanın dayandığı hüküm — "EC şarj limitini uyanışta geri
alıyor" (6 Eyl) — bu s2idle döngüsünde **tekrarlanmadı**; limit korunmuştu.
Bu makinede hibernate kapalı (`nohibernate`, `power.nix`, 24 Ağu 2026), yani
o gözlem başka bir uyku tipinden de gelemez. 6 Eyl gözlemi `aorus_laptop`'ın
`charge_limit` geri okumasına dayanıyordu ve o sürücünün yanlış bildirdiği
artık ölçüldü (Ölçüm 7).

**Hüküm "ölçüldü" → "tekrarlanmadı" oldu.** Kanca kalıyor: bedeli bir okuma
ve gerçekten geri alınan bir durum olursa yakalıyor — ama kanıtlanmış bir
gereklilik olarak sunulmuyor.

### ✅ Ölçüm 9 — `platform_profile` legacy düğüm davranışı — **YAPILDI 7 Eyl 2026**

Adım 5'in tek riski şuydu: ikinci bir handler kaydetmek
`/sys/firmware/acpi/platform_profile_choices`'i daraltıp `low-power`'ı
düşürür mü? Düşürürse `power-display.nix`'in pildeki `power-saver` otomatiği
kırılır ve 4.28 W boşta bütçesi vurulur.

**Ayırt edici deney** (kümemizden `low-power` geçici çıkarıldı):

| koşu | `amd-pmf` | `aero` | legacy |
|---|---|---|---|
| A (üst küme) | `lp b p` | `lp b bp p` | `lp b bp p` |
| B (`lp` yok) | `lp b p` | `b bp p` | `b bp p` |

**İki hüküm düzeldi:**

1. **Legacy "kesişim" değil.** Kesişim olsaydı B'de `balanced-performance` de
   düşerdi. Legacy her iki koşuda tam olarak *bizim* kümemizi gösterdi.
   Ama sonuç aynı: kümemizden `low-power` çıkarsa legacy'den de kayboluyor →
   **üst-küme kısıtı gerçekten load-bearing**, artık ölçülmüş.
2. **Fazladan seçenek `amd-pmf`'e sızıyor.** Legacy'ye `balanced-performance`
   yazıldı: kabul edildi, ve `amd-pmf` kendi `choices`'inde olmamasına rağmen
   onu `profile` olarak okudu. SMU tarafında ne yaptığı ölçülmedi.
   → `balanced-performance` **düşürüldü**; küme artık `amd-pmf`'inkiyle birebir.

**Ayrıca doğrulandı:** dört profil de yazılıp geri okundu, `max-power`
reddedildi, legacy yazımı **her iki handler'a** gidiyor, `powerprofilesctl`
zinciri sürüyor.

**Kapanış ölçümü:** 3 seçenekli sürümle legacy düğüm modülsüz hâle
**BİREBİR AYNI** çıktı (`low-power balanced performance`), `balanced-performance`
legacy'de yok, üç profil de iki handler'a birden gidiyor. Betik:
`scripts/verify-profile.sh`.

**Yan etki (kabul edildi):** modül yüklenince legacy `profile` geçici olarak
`custom` okuyor, çünkü `WMBC`'de `0xED` okuması yok ve sürücü "bilmiyorum"
demeyi uydurmaya tercih ediyor. İlk profil yazımında çözülüyor.

### ✅ Ölçüm 10 — boot yarışı — **YAPILDI 8 Eyl 2026, İKİ REBOOT**

`~/nixos-zixar` geçişinden sonraki ilk gerçek boot, elle `insmod`'un
göremediği bir hatayı ortaya çıkardı.

| boot | log | sonuç |
|---|---|---|
| 1 (hatalı) | `BAT1 bulunamadi` → 1 sn sonra `ACPI: battery: Slot [BAT1]` | şarj limiti düğümü **hiç oluşmadı** |
| 2 (düzeltilmiş) | `BAT1 henuz yok (boot yarisi)` → 1 sn sonra `sarj limiti hazir … = 60%` | ✅ bağlandı |

Sürücü `boot.kernelModules` ile ACPI pil sürücüsünden **önce** yükleniyor.
Çözüm sınırlı gecikmeli yeniden deneme; kendini durdurduğu ayrıca ölçüldü
(sahte `BATYOK` adıyla: pes ettikten sonra 4 sn boyunca sıfır yeni satır).

**Aynı boot ikinci bir hata daha verdi:** `Failed to resolve unit specifiers
in 'Pil şarj limiti %60'` — systemd `%` ile başlayanı birim belirteci sanıyor.
`nix` modülünde `%%` kaçırması eklendi.

**Ders:** ağaç-dışı bir sürücüde *"elle `insmod` ile çalıştı"* bir doğrulama
değildir. Boot sırası, elle yüklemenin hiç kurmadığı bir durum.
