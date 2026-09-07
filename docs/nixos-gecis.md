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

Beş dosya. Hiçbiri henüz uygulanmadı.

### 4.1 `system/arch/aerox16/wmi.nix`

- `aorus-laptop` türetmesi ve `boot.extraModulePackages` / `boot.kernelModules`
  girdileri → `aero-eg61h` türetmesiyle değişir (kaynak `~/aero-eg61h/kernel`;
  ağaç-dışı derleme deseni **birebir aynı**: `kernel.stdenv` +
  `kernelModuleMakeFlags` + `KDIR=${kernel.dev}/…`).
- `boot.blacklistedKernelModules = [ "aorus-laptop" ]` — sürücü zaten yüklüyse
  bağlanmayı reddediyor, ama blacklist niyeti açık kılar.
- `gigabyte-power-profile.service`: `P=/sys/devices/platform/aorus_laptop` →
  `F=$(echo /sys/bus/wmi/devices/ABBC0F75-*/fan_mode)`, `FAN=1` → `FAN=balanced`.
  `game-perf` koşulu ve ACBT `acpi_call` bloğu **aynen kalır**.
- `fan-mode-cycle.service`: `case` sayı yerine isim üzerinden dönsün.
  Bildirim metinleri zaten Türkçe isimler kullanıyor, sadece `next` değerleri
  isim olur.
- `gigabyte-charge-limit.service`: `P/charge_mode` → `acpi_call` (§3),
  `P/charge_limit` → `/sys/class/power_supply/BAT1/charge_control_end_threshold`.
  **`powerManagement.resumeCommands`'a bağlamaya artık gerek yok** — sürücünün
  kendi uyanış kancası var (ama kanca gerçek bir suspend ile doğrulanmadan
  servisi kaldırmayın; ikisi bir arada zararsız).

### 4.2 `system/kernel/sched.nix`

`game-perf.service` içindeki `F=/sys/devices/platform/aorus_laptop/fan_mode` /
`echo 5` → yeni yol + `echo turbo`. `0xED 2` yazan `acpi_call` bloğu aynen kalır.

### 4.3 `modules/hardware/gaming.nix`

Aynı yol, aynı değişiklik (`sched.nix` ile ikiz görünüyor — hangisinin canlı
olduğu geçiş sırasında doğrulanmalı; import listesinde olmayan bir dosya
**test edilmemiş** demektir, `CLAUDE.md`'nin `dns.nix` dersi).

### 4.4 Tanılama betikleri

`scripts/tclt-probe.sh`, `scripts/idle-baseline.sh`, `scripts/diag-game.sh` —
`fan_mode` okuma yolları. Bunlar bozulsa sistem çalışır, ama aynı anda
düzeltmek ucuz.

### 4.5 `MAINTAINERS` + `Documentation/aerox16/wmi-ec.md`

Sürücü değişimi ve §2'deki numaralandırma tuzağı işlenmeli. 16 Ağu ölçüm
tablosuna "aorus numaralandırmasıyla alındı, desen eşlemesi belirsiz" notu.

## 5. Geçişten önce kalan iki iş

1. **Uyanış kancası doğrulanmalı.** Sürücünün `.resume`'u gerçek bir
   suspend/resume ile test edilmedi. Test: şarj limitini yaz, uyut, uyandır,
   `sudo dmesg | grep uyanis`. Satır yoksa yol `register_pm_notifier` olacak.
   *(Geçişi engellemez: eski servis `resumeCommands`'da bırakılabilir.)*
2. **`0xED` perf profili sürücüye alınmalı mı?** Şu an `acpi_call` ile
   yapılıyor ve çalışıyor. K1 = (a′) bunu `platform_profile`'a taşımayı
   öngörüyor (adım 5). Geçiş bunu **beklemek zorunda değil** — `acpi_call`
   yolu bozulmadan kalıyor.

## 6. Geri dönüş

Tek satır: `boot.blacklistedKernelModules`'ü kaldır, `aorus-laptop`'ı
`boot.kernelModules`'a geri koy, rebuild. Sürücülerin ikisi de ağaç-dışı
modül, ikisi de aynı desenle derleniyor. Servislerin eski hâli git geçmişinde.

Canlı (rebuild'siz) geri dönüş, geçiş sırasında bir şey ters giderse:

```bash
sudo rmmod aero_eg61h && sudo modprobe aorus_laptop
sudo systemctl start gigabyte-power-profile gigabyte-charge-limit
```
