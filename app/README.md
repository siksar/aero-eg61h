# `app/` — kullanıcı alanı

Cargo workspace. **Katman katman, her katman tek başına koşturulabilir.**

| crate | ne | bağımlılık | yetki |
|---|---|---|---|
| `aero-sysfs` | veri katmanı — sürücünün sysfs yüzeyini okur | **yok** (std) | **yok** |
| `aero-ctl` | durum dökümü (tanılama + `aero-sysfs`'in testi) | `aero-sysfs` | **yok** |
| `aero-control` | GUI (libcosmic) — 7 panel | `aero-sysfs` + `libcosmic` | **yok** (köprüler polkit'li) |

## Neden yetki gerekmiyor

Sürücünün sunduğu dört değerin dördü de **dünyaya-okunur** (ölçüldü, 7 Eyl 2026):

```
/sys/class/hwmon/hwmonN/temp1_input                       r--r--r--
/sys/class/power_supply/BAT1/charge_control_end_threshold rw-r--r--
/sys/bus/wmi/devices/ABBC0F75-…/fan_mode                  rw-r--r--
/sys/firmware/acpi/platform_profile                       rw-r--r--
```

Yani salt-okunur GUI **root olmadan** çalışır. Yazma yolu da öyle: var olan
polkit'li köprüleri çağırıyor, kendi daemon'ını kurmuyor — aşağıda
"Yazma yolu" bölümü.

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
./run.sh            # GUI'yi calistirir (LD_LIBRARY_PATH tuzagi icin — asagi bak)
```

PATH'teki `cargo` bir rustup kabuğu ve varsayılan toolchain yok; `build.sh`
araç zincirini flake'ten alır (`kernel/build.sh` ile aynı desen). `clippy` ve
`rustfmt` de aynı kabuğa takılıyor — ikisi de `.gcroots/` altında, flake'ten
ve `rustc` ile **aynı sürümde**.

> **Biçim geleneği yalnız `aero-control` için.** O kritin dosyaları
> rustfmt-temiz tutuluyor (8 Eyl'de ölçüldü: HEAD'de 0 fark); `aero-sysfs` ve
> `aero-ctl` değil, ve `curves.rs` **bilerek** değil — üretilen tablo elle
> hizalanmış sütunlar taşıyor, rustfmt onları dağıtır ve üreteçle her koşuda
> çatışır. Yani: `aero-control`'a dokunduysan `rustfmt`, ötekilere dokunma.

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

## `aero-control` — GUI

Yedi panel: **Ön Ayarlar** · **Durum** · **Fan ve Termal** · **Fan Eğrisi** ·
**Güç ve Performans** · **Pil** · **Hakkında**

### GUI'yi çalıştırmak — `./run.sh`, düz `./target/…/aero-control` DEĞİL

İkiliyi doğrudan çalıştırmak **anında panikliyor**:

```
Create event loop: Os(... error: NoWaylandLib)
```

`winit`, `libwayland-client.so`'yu çalışma anında `dlopen` ediyor; Nix dışında
derlenmiş ikilinin rpath'inde o yok ve NixOS'ta `/usr/lib` de yok. `run.sh`
`LD_LIBRARY_PATH`'i `.gcroots/gui-env`'e kuruyor ve gerekirse o env'i kendisi
kuruyor (flake'te bir attribute değil, elle kurulmuş bir `buildEnv` — ifade
betiğin içinde ve 8 Eyl 2026'da çalıştırılarak doğrulandı).

### Nav simgeleri gömülü — üçü zorunlu olarak

Kurulu ikon temasında (Pop/Cosmic) `temperature-symbolic`,
`power-profile-performance-symbolic` ve herhangi bir çizgi-grafiği adı **yok**
(8 Eyl 2026'da tek tek arandı), o yüzden o üç satır nav'da **boş** duruyordu.
Simgeler artık `data/icons/{fan,power,fan-curve}-symbolic.svg` ve `include_bytes!`
ile ikiliye gömülü. Diğer dördü temadan çözülüyor, onlar gömülü değil.

### Tema — kendi paleti YOK

`libcosmic`, `~/.config/cosmic/com.system76.CosmicTheme.*` dosyalarını **canlı**
okuyor: vurgu rengi, gece/gündüz, yoğunluk, font, ikon teması bedava geliyor ve
COSMIC Ayarlar'dan değiştirince anında uyuyor. Bu depoda `~/.config/cosmic`'e
**yazmak yasak** — yalnız okunuyor.

### Örnekleme — boşta güç kuralı

Yalnız uygulama açıkken, ve görünür panele göre: Durum/Fan'da 2 sn, diğerlerinde
5 sn. Kapanınca arkada hiçbir şey kalmıyor.

### Bilerek OLMAYAN düğmeler

| yok olan | çünkü |
|---|---|
| Fan hızı kaydırıcısı | bu firmware'de fan hızı ayarlanamıyor (duty yazmaçları inert, üç kanıt) |
| Soket sıcaklığı | `SKTC` ölü kanal — tam yükte CPU 91 °C iken bile 0 |
| Klavye aydınlatma | `KBLL` ölü yazmaç — yazılıyor, tutuyor, görsel etkisi yok |

Bunların yerine tek satırlık açıklama duruyor. Çalışmayan bir düğüm koymak,
düğüm koymamaktan kötüdür.

### libcosmic paketleme

crates.io'da yok, git bağımlılığı. Pin `cosmic-files`'ın epoch-1.6.0
sürümünden alındı (`c1897c01`) — nixpkgs'in paketlediği, bilinen-iyi bileşim;
kendi başımıza rev seçmiyoruz. `a11y` özelliği BİLEREK yok (upstream
"a11y feature crashes" diyor).

594 crate, ilk derleme ~2 dk. Nix paketi: `nix/aero-control.nix`
(`libcosmicAppHook` + `cargoLock.outputHashes`).

## Yazma yolu (8 Eyl 2026)

**D-Bus daemon YOK** — karar ölçümle verildi. Var olan iki köprü kullanılıyor:

| eylem | köprü | yetki |
|---|---|---|
| Performans profili | `powerprofilesctl set` (PPD) | yok — PPD zaten polkit'li |
| Fan modu | `systemctl start aero-set-fan@<mod>` | yok — modülün polkit kuralı |
| Şarj limiti | `systemctl start aero-set-charge@<yüzde>` | yok — aynı kural |

**Doğrulama ayrıcalıklı tarafta.** `%i` kullanıcıdan geliyor, o yüzden beyaz
listeyi systemd birimi tutuyor. Rust tarafındaki aralık denetimi yalnız hızlı
geri bildirim için; kaldırılsa sistem güvensiz olmaz, yalnız hata mesajı geç
ve çirkin olur.

```bash
aero-ctl set fan turbo
aero-ctl set charge 80
aero-ctl set profile performance
```

Başarıda **yazdığını değil sistemin geri okuduğunu** bildirir.

### GUI: Ön Ayarlar paneli

Beş isimlendirilmiş paket (`PLAN.md` §5) — her biri fan modu + profili tutarlı
bir bütün olarak kurar. Ham sayı yok; amacı bir şeyi bozamamak.

| ön ayar | fan | profil |
|---|---|---|
| Sessiz | `quiet` | `low-power` |
| Dengeli | `balanced` (mod 4) | `balanced` |
| Duyarlı | `responsive` (mod 0) | `balanced` |
| Performans | `gaming` | `performance` |
| Maksimum | `turbo` | `performance` |

Artı şarj limiti kaydırıcısı (bırakılınca yazar).

Yazma hatası **yutulmuyor** — üstte kapatılabilir bir şeritte aynen gösteriliyor.

### GUI: Fan Eğrisi paneli (8 Eyl 2026)

`duty = f(sıcaklık)`, **basamak** çizimi. `PLAN.md` §4.1'in karşılığı.

Veri firmware imajından (`aero-sysfs/src/curves.rs`, üreteç `gen-curves.py`),
yani panel **hiçbir donanıma dokunmuyor**: açıkken fazladan tek bir EC okuması
yapmıyor, canlı imleç zaten toplanan anlık görüntüyü kullanıyor.

| ne | nasıl |
|---|---|
| çizim | **basamak**, interpolasyon YOK — ara davranış ölçülmedi |
| iki eğri | tek mod görünümünde Fan 1 düz, Fan 2 **kesik** çizgi |
| karşılaştırma | "beş modu birden" — tek fan, mod başına bir renk |
| renk sözlüğü | **renk = mod, kesiklik = fan 2**; renkler COSMIC temasından |
| canlı katman | CPU sıcaklığında düz dikey imleç + kesişim noktaları |
| fare | grafiğin üstünde gezinince kesik imleç + o sıcaklıktaki duty'ler |
| efsane | tuvalin **içinde**, sol üstte — duty tavanı %63 olduğu için orası her modda boş |
| ham tablo | 14×3, kaynak ofseti ve **kural kaydı** (imajdan aynen) |

**Panelin tek iddiası** "grafikte gördüğün şekil, uygulamanın hesabının ta
kendisi" ve bu iddia bir testle bağlı: `egri::tests::cizim_hesapla_ayni`
üç farklı eksen ucu × 10 eğri × her tam sayı sıcaklıkta poligonun değerini
`Egri::duty` ile karşılaştırıyor.

#### Bu panelde bilerek OLMAYAN iki şey

| yok olan | çünkü |
|---|---|
| «EC'den doğrula» düğmesi | EC'deki canlı eğriyi `EIDR` ile okumak mümkün ama **zaman aşımı bayrağı (`ECTE`) hiçbir ACPI yolundan görünmüyor** — bayat veri geçerli cevaptan ayırt edilemiyor (`kernel/README.md`) |
| eğri düzenleme | veri portu salt okunur (ölçüldü); mod seçimi yalnız hangi eğrinin yükleneceğini belirler |

İkisinin de gerekçesi **ekranda** yazıyor, sessizce eksik değiller.

#### Kural kaydı — kanıt, çıkarım değil

`Egri` artık kendisini seçen 8 baytlık kural kaydını taşıyor
(`{ b0, 00, b2, iç_mod, 00, FF, adres_hi, adres_lo }`, kural tablosu `0x0616E`,
`~/ecscope/docs/firmware-8051.md` §4.2). Kayıt **yeniden kurulmuyor**: üreteç
imajda adres alanı tabloyla eşleşen tek kaydı arıyor, `mod`/`b0` alanlarını
beklentiyle karşılaştırıyor, eşleşme tek değilse duruyor. Böylece "bu eğri
nereden geliyor" sorusunun cevabı bir alıntı.

> **Üreteç tuzağı kapandı.** `curves.rs`'in testleri daha önce dosyanın sonuna
> **elle** eklenmişti ve `gen-curves.py` onları emitlemiyordu — bir sonraki
> `--rust` koşusu 7 testi sessizce silecekti. Testler artık şablonun içinde.
