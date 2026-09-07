# `aorus_laptop` → `aero_eg61h` geçişi — `~/nixos-zixar` planı

> 7 Eylül 2026 · **Bu bir plandır, uygulanmadı.** `~/nixos-zixar`'a habersiz
> dokunulmuyor; bu dosya değişikliği tarif ediyor, kullanıcı uygulanmasına
> ayrıca karar verecek.
>
> Kullanıcı kararı (7 Eyl): *"bundan sonra aorus laptop wmi yerine sadece bunu
> kullanacagiz."* → K4 `(a) elle rmmod` yerine **`(b) kalıcı geçiş**.

## 1. Özellik eşitliği — geçişin ön koşulu

`~/nixos-zixar` `aorus_laptop`'tan **iki** sysfs alanı kullanıyor. İkisinin de
karşılığı artık var:

| `aorus_laptop` | kullanan | `aero_eg61h` karşılığı | durum |
|---|---|---|---|
| `fan_mode` | `gigabyte-power-profile.service`, `fan-mode-cycle.service`, `game-perf.service` (`sched.nix`) | `/sys/bus/wmi/devices/ABBC0F75-8EA1-11D1-00A0-C90629100000-2/fan_mode` | ✅ adım 4 |
| `charge_limit` | `gigabyte-charge-limit.service` | `/sys/class/power_supply/BAT1/charge_control_end_threshold` | ✅ adım 3 |
| `charge_mode` | aynı servis | **yok** — §3'e bak, `acpi_call` ile kapanıyor | ⚠️ |

Geri kalan her şey (ACBT dGPU boost bütçesi `0x4C`, perf profili `0xED`) zaten
**`acpi_call` üzerinden** yapılıyor, `aorus_laptop` ile ilgisi yok. `acpi_call`
kalıyor.

Salt-okuma kullanan üç betik (`scripts/tclt-probe.sh`, `scripts/idle-baseline.sh`,
`scripts/diag-game.sh`) da `fan_mode` okuyor — onların yolu da değişmeli, ama
bozulsalar bile sistem çalışır (tanılama betikleri).

## 2. ⚠️ EN ÖNEMLİ NOKTA: sayıları BİREBİR ÇEVİRMEYİN

`aorus_laptop`'ın `fan_mode` numaraları `PECM+0x2C` desenlerine eşlenmiyor.
**Mekanizma 7 Eyl 2026'da ölçüldü** (önceki hüküm bir çıkarımdı ve mekanizma
kısmı yanlıştı).

**Deney.** `aorus_laptop` yüklüyken `fan_mode`'a 1/2/5/4 yazıldı; her yazımdan
önce ve sonra `0x2C`'nin dört biti `WMBC 0x57/0x71/0x67/0x6A` ile okundu.

| yazılan | `0x2C` öncesi → sonrası | ne yaptı |
|---|---|---|
| `1` | `0x09` → `0x09` | b0 (`CRAF`) kurdu, **b3'e dokunmadı** |
| `2` | `0x09` → `0x0a` | b1 (`FANB`) kurdu, b0'ı temizledi, b3'e dokunmadı |
| `5` | `0x0a` → `0x0c` | b2 (`TENF`) kurdu, b1'i temizledi, b3'e dokunmadı |
| `4` | `0x0c` → `0x04` | **b3'ü (`ADJF`) temizledi**, b2'yi bıraktı |

Yani b0/b1/b2 birbirini dışlayan bir grup olarak yönetiliyor, ama **b3 desenin
parçası sayılmıyor** — ve `fan_mode = 4` bir mod değil, "ADJF'yi kapat" işlemi.

**Sonuç ADJF'nin o anki durumuna bağlı:**

| yazılan | ADJF=1 iken | ADJF=0 iken |
|---|---|---|
| `1` "sessiz" | `0x09` = **mod4** ✗ | `0x01` = quiet ✓ |
| `2` "gaming" | `0x0a` = **tanınmıyor → varsayılan** ✗ | `0x02` = gaming ✓ |
| `5` "turbo" | `0x0c` = turbo ✓ | `0x04` = **tanınmıyor → varsayılan** ✗ |
| `4` "dengeli" | `0x04` = **varsayılan** ✗ | `0x04` = **varsayılan** ✗ |

**Dördünün de doğru çalıştığı bir ADJF durumu yok.**

### İki somut kayıp (geçişin aciliyeti)

1. **Oyun turbosu güvenilir değil.** `game-perf.service` `fan_mode = 5`
   yazıyor. Bu yalnız ADJF=1 iken `0x0C` (turbo) veriyor; ADJF=0 iken `0x04`,
   yani **varsayılan**. Sessizce.
2. **Süper+M döngüsü ADJF'yi sıfırlıyor.** Döngü `4→1→2→5`; ilk adım "4" tam
   olarak b3'ü temizleme işlemi. Ondan sonra hem döngünün kendi "Turbo"su hem
   de sonraki `game-perf` turbosu `0x04` = varsayılan veriyor. Yani **Süper+M'yi
   bir kez kullanmak turbo'yu erişilemez yapıyor** (bir sonraki `1` yazımına
   kadar ADJF 0 kalıyor ve `1` de onu geri kurmuyor).

Bizim sürücüde bu mümkün değil: hedef deseni dört seçiciyle tam yazıp yine dört
seçiciyle geri okuyoruz, uyuşmazlıkta `-EIO` dönüyoruz.

**Geçiş sonucu:** bugünkü davranışı korumak için AC/BAT varsayılanı
**`balanced`** olmalı — makinenin aylardır gerçekten koştuğu mod bu
(`fan_mode = 1` + ADJF=1 → `0x09`).

> **Yan sonuç — `wmi.nix`'teki 16 Ağu 2026 ölçüm tablosu şüpheli.** O tablo
> `aorus_laptop`'ın numaralandırmasıyla alınmış, yani hangi `0x2C` desenini
> ölçtüğü ADJF geçmişine bağlı ve belirsiz. **Geçişten sonra bizim
> isimlerimizle yeniden ölçülmeli.**

Önerilen eşleme (isimden isme, sayıdan sayıya değil):

| bugünkü niyet | `aorus` no | `aero_eg61h` adı | `0x2C` |
|---|---|---|---|
| AC/BAT varsayılanı | `1` (→ gerçekte `0x09`) | **`balanced`** | `0x09` |
| oyun turbosu (`game-perf`) | `5` | **`turbo`** | `0x0C` |
| Süper+M döngüsü | `4 → 1 → 2 → 5` | `balanced → quiet → gaming → turbo` | `09→01→02→0C` |

## 3. `charge_mode` boşluğu — ölçüldü, kapanıyor

`gigabyte-charge-limit.service` `echo 1 > charge_mode` yazıyor ve dosyadaki
yorum "charge_limit yalnız custom charge_mode'da (1) çalışır" diyor.
Sürücümüz `charge_mode` sunmuyor (`WMBD 0x64` / BCPS — anlamı DSDT'den
doğrulanamıyor, standart karşılığı yok).

**Ölçüm (7 Eyl, `acpi_call` ile):** `WMBC 0x64` = **`0x04`**.
Yani `aorus`'un `charge_mode = 1`'i EC'de BCPS = **4** üretiyor — bu da bire bir
eşleme olmadığının ikinci örneği.

**Çözüm, tahminsiz:** `charge-limit` servisi BCPS'i `acpi_call` ile doğrudan
yazsın (`acpi_call` zaten kalıyor):

```sh
echo '\_SB.PCI0.AMW0.WMBD 0 0x64 4' > /proc/acpi/call
```

**Doğrulanmamış olan:** BCPS'in reboot'ta korunup korunmadığı. Bugün 4 okuyor
ama bunu `aorus`'un boot servisi mi yazdı, yoksa EC mi koruyor bilinmiyor.
Yukarıdaki satır her boot'ta yazdığı için bu soru **geçişi engellemiyor** —
ama merak edilirse ölçümü basit: aorus blacklist'liyken cold boot sonrası
`WMBC 0x64` oku.

## 4. Uygulanacak değişiklik (`~/nixos-zixar`)

**NixOS modülü yazıldı ve doğrulandı: `nix/aero-eg61h.nix`.** Bu sayede
`~/nixos-zixar` tarafındaki iş üç küçük parçaya indi.

### 4.1 Flake girdisi + import

```nix
# flake.nix
inputs.aero-eg61h = { url = "git+file:///home/zixar/aero-eg61h"; flake = false; };

# configuration.nix (ya da system/arch/aerox16/wmi.nix)
imports = [ "${inputs.aero-eg61h}/nix/aero-eg61h.nix" ];
hardware.aero-eg61h.enable = true;
```

Varsayılanlar bugünkü davranışı korur: `fanMode.ac = fanMode.battery = "balanced"`,
`chargeLimit = 60`, `gpuBoost.ac = 10`, `gpuBoost.battery = 0`.

Modül şunları kendisi yapıyor: sürücüyü derleyip yükler, `aorus-laptop`'ı
blacklist'ler, `acpi_call`'ı korur, üç servisi (`aero-power-profile`,
`aero-charge-limit`, `aero-fan-cycle`) ve ACAD udev kuralını + polkit
kuralını kurar.

### 4.2 `system/arch/aerox16/wmi.nix`'ten SİLİNECEKLER

- `aorus-laptop` türetmesi (satır ~38-66)
- `boot.extraModulePackages` / `boot.kernelModules` satırları — modül devraldı
- `gigabyte-power-profile.service` — `aero-power-profile` devraldı
- `gigabyte-charge-limit.service` — `aero-charge-limit` devraldı
- `fan-mode-cycle.service` + polkit kuralı — `aero-fan-cycle` devraldı
- ACAD udev kuralı — modül kendi kuralını kuruyor

**KALACAKLAR:** Fn tuşu `hwdb` kuralı (`KEYBOARD_KEY_7006f=reserved`) ve
dosyadaki ölçüm/karar yorumları (özellikle 16 Ağu tablosu — §2'nin uyarısıyla).

Süper+M keybind'ı `fan-mode-cycle.service` yerine **`aero-fan-cycle.service`**
göstermeli (`home/desktop/wm/binds.lua` ya da COSMIC kısayolları).

### 4.3 `system/kernel/sched.nix`

`game-perf.service` içindeki iki satır:

```diff
-      F=/sys/devices/platform/aorus_laptop/fan_mode
-      [ -w "$F" ] && echo 5 > "$F"
+      F=$(echo /sys/bus/wmi/devices/ABBC0F75-*/fan_mode)
+      [ -w "$F" ] && echo turbo > "$F"
```

`0xED 2` yazan `acpi_call` bloğu aynen kalır. **Bu değişiklik §2'deki 1.
kaybı da düzeltiyor:** artık turbo gerçekten turbo.

> `modules/hardware/gaming.nix` de aynı satırları taşıyor ama **hiçbir yerden
> import edilmiyor** (7 Eyl'de doğrulandı; `configuration.nix` yalnız
> `system/kernel/sched.nix`'i alıyor). Eski ağaç düzeninden kalma ölü dosya.
> **Düzenlenmeyecek** — ölü bir dosyayı güncellemek onu bakımlı gösterir.
> Ya silinmeli ya da başına "import edilmiyor" notu düşülmeli.

### 4.4 Tanılama betikleri

`scripts/tclt-probe.sh`, `scripts/idle-baseline.sh`, `scripts/diag-game.sh` —
`fan_mode` okuma yolları. Bozulsalar sistem çalışır; aynı anda düzeltmek ucuz.
`diag-game.sh`'deki `(5 = turbo, oyunda beklenen)` metni `turbo` olmalı.

### 4.5 `MAINTAINERS` + `Documentation/aerox16/wmi-ec.md`

Sürücü değişimi ve §2'deki eşleme tuzağı işlenmeli. 16 Ağu ölçüm tablosuna
"`aorus_laptop` numaralandırmasıyla alındı; hangi `0x2C` desenini ölçtüğü ADJF
geçmişine bağlı ve belirsiz — yeniden ölçülmeli" notu.

## 4b. Modülün doğrulaması (7 Eyl 2026)

`~/nixos-zixar`'a **dokunulmadan**, onun kendi yapılandırması
`extendModules` ile genişletilerek test edildi:

| ne | sonuç |
|---|---|
| Modül evali | ✅ üç servis kuruldu, `aorus-laptop` blacklist'e eklendi |
| Çekirdek modülü türetmesi | ✅ derlendi, `vermagic` çalışan çekirdekle birebir |
| Üç servis betiği | ✅ derlendi (yani `bash -n` denetiminden geçti) |
| `aero-fan-cycle` canlı | ✅ `balanced → quiet → gaming → turbo → balanced` |
| `aero-charge-limit` canlı | ✅ rc=0, limit %60 |
| `aero-power-profile` canlı | ✅ rc=0, pilde `balanced` |

## 5. Geçişten önce kalan iş

**Kalmadı.** Adım 5'teki iki açık iş de 7 Eyl'de kapandı:

1. ~~Uyanış kancası doğrulanmalı~~ → **DOĞRULANDI** (19:06):
   `aero_eg61h: uyanis: sarj limiti 60% korunmus, dokunulmadi`.
   `driver->pm` WMI bus'ında tetikleniyor. *(Yan bulgu: kancanın dayandığı
   "EC uyanışta geri alıyor" hükmü bu döngüde tekrarlanmadı — `PLAN.md`
   Ölçüm 8.)*
2. ~~`0xED` sürücüye alınmalı mı~~ → **hayır, geçişi beklemiyor.** Şu an
   `acpi_call` ile yapılıyor ve modül o yolu koruyor. `platform_profile`
   (K1 = a′) ayrı bir adım (§10 adım 5) ve geçişten bağımsız.

Yani teknik engel yok; kalan tek şey `~/nixos-zixar`'daki 44 bekleyen
değişikliğin arasına bunu ne zaman katmak istediğin.

## 6. Geri dönüş

Tek satır: `boot.blacklistedKernelModules`'ü kaldır, `aorus-laptop`'ı
`boot.kernelModules`'a geri koy, rebuild. Sürücülerin ikisi de ağaç-dışı
modül, ikisi de aynı desenle derleniyor. Servislerin eski hâli git geçmişinde.

Canlı (rebuild'siz) geri dönüş, geçiş sırasında bir şey ters giderse:

```bash
sudo rmmod aero_eg61h && sudo modprobe aorus_laptop
sudo systemctl start gigabyte-power-profile gigabyte-charge-limit
```
