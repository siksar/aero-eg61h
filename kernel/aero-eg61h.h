/* SPDX-License-Identifier: GPL-2.0-only */
/*
 * aero_eg61h — Gigabyte AERO X16 1VH (SKU EG61VH) platform sürücüsü
 *
 * Ortak tanımlar: WMI seçicileri, sürücü durumu, EC erişim sarmalayıcıları.
 *
 * Bu dosyadaki her seçici değeri DSDT'den okundu ve canlı ölçümle doğrulandı.
 * Kaynaklar:
 *   ~/ecscope/docs/aero-x16-catalogue.md   DSDT WMBD/WMBC kataloğu (satır no'lu)
 *   ~/ecscope/docs/yazma-ve-ic-uzay.md     6 Eyl 2026 canlı yazma ölçümleri
 *   ~/ecscope/docs/firmware-8051.md        EC firmware statik analizi
 *   ~/ecscope/lib/selectors.tsv            risk sınıfları
 */

#ifndef _AERO_EG61H_H
#define _AERO_EG61H_H

#include <linux/mutex.h>
#include <linux/types.h>
#include <linux/wmi.h>

/*
 * WMI GUID'leri — DSDT _WDG tamponundan çözüldü (dsdt.dsl.txt 8977-8989).
 *
 *   ABBC0F6C  "AC"  flags 0x01 (expensive) veri bloğu — WQAC/WSAC ikisi de TASLAK,
 *                   sabit 1 döndürüyor, hiçbir EC alanına dokunmuyor. Üstelik
 *                   kapıyı tutan WCAC yanlış sırada çağrılırsa ACPI Fatal atıyor
 *                   (DSDT 9059/9066). SÜRÜCÜ BU GUID'E BAĞLANMAZ.
 *   ABBC0F6F  "BC"  flags 0x02 (metot) — WMBC, okuma yolu
 *   ABBC0F75  "BD"  flags 0x02 (metot) — WMBD, yazma yolu
 *   ABBC0F72        flags 0x08 (olay)  — notify 0xD2, _WED 4 baytlık DEVS döner
 */
#define AERO_WMI_GUID_WMBC	"ABBC0F6F-8EA1-11D1-00A0-C90629100000"
#define AERO_WMI_GUID_WMBD	"ABBC0F75-8EA1-11D1-00A0-C90629100000"
#define AERO_WMI_GUID_EVENT	"ABBC0F72-8EA1-11D1-00A0-C90629100000"

/* _WDG'deki olay bildirim kodu; _WED yalnız bu değerde DEVS döndürür (DSDT 9721) */
#define AERO_WMI_EVENT_ID	0xD2

/*
 * ---------------------------------------------------------------------------
 * OKUMA seçicileri — WMBC(0, sel, arg)
 * ---------------------------------------------------------------------------
 * Hepsi salt okuma; yan etkisi yok. Tek istisna 0x68 (aşağıda, kullanılmıyor).
 */
#define AERO_RD_CPU_TEMP	0xE1	/* CTMP  ECMM+0xB0, °C            */
#define AERO_RD_FAN1_RPM	0xE4	/* RPM1  PECM+0x13, 16 bit        */
#define AERO_RD_FAN2_RPM	0xE5	/* RPM2  PECM+0x15, 16 bit        */
#define AERO_RD_CHARGE_LIMIT	0x65	/* BCPC  PECM+0x05, %            */
#define AERO_RD_AC_ONLINE	0xA2	/* ACST == 0x04 boole sonucu     */
#define AERO_RD_LID		0xEF	/* LIDF bit-DEĞİLİ — 0/1 DEĞİL, ters tamsayı */
#define AERO_RD_FAN_CRAF	0x57	/* PECM+0x2C bit0  sessiz        */
#define AERO_RD_FAN_FANB	0x71	/* PECM+0x2C bit1  gaming        */
#define AERO_RD_FAN_TENF	0x67	/* PECM+0x2C bit2                */
#define AERO_RD_FAN_ADJF	0x6A	/* PECM+0x2C bit3                */

/*
 * ---------------------------------------------------------------------------
 * YAZMA seçicileri — WMBD(0, sel, val)
 * ---------------------------------------------------------------------------
 * Buradaki listeye YALNIZ selectors.tsv'de GUVENLI ya da DIKKAT sınıfında olan
 * ve sürücünün gerçekten kullanacağı seçiciler girer.
 */
#define AERO_WR_FAN_CRAF	0x57	/* GFAN=0; CRAF=val                       */
#define AERO_WR_FAN_FANB	0x71	/* GFAN=0; FANB=val                       */
#define AERO_WR_FAN_TENF	0x67	/* TENF=val                               */
#define AERO_WR_FAN_ADJF	0x6A	/* ADJF=val                               */
#define AERO_WR_CHARGE_LIMIT	0x65	/* BCPC=val (%) — UYANIŞTA EC GERİ ALIYOR */
#define AERO_WR_PERF_PROFILE	0xED	/* 0-3: CPU PL1/2/3 + dGPU bütçesi paketi */

/*
 * ÖLÜ KANAL — ölçüldü, sunulmayacak
 *
 *   0xE2 / 0xE3  SKTC (ECMM+0xB4, "soket sıcaklığı")
 *
 * İki seçici de aynı alanı okuyor ve o alan bu makinede HEP SIFIR.
 * 7 Eyl 2026, iki bağımsız koşu:
 *   - boşta:  WMBC 0xE2 = 0, aorus_laptop temp2_input = 0 (aynı anda)
 *   - tam yük: CPU 91 °C, fanlar 3333/3703 rpm dönerken  WMBC 0xE2 = 0
 * Yani okuma doğru, alan boş. hwmon'a temp2 GİRMİYOR.
 *
 * aorus_laptop bu makinede temp2 VE temp3 sunuyor, ikisi de sıfır okuyor —
 * pwm1/pwm2 ile aynı hata sınıfı: ölçülmemiş bir kanalı varmış gibi göstermek.
 */

/*
 * ---------------------------------------------------------------------------
 * ASLA KULLANILMAYACAK seçiciler — buraya bir #define eklemek bile yanlış sinyal
 * ---------------------------------------------------------------------------
 *   0x63 0x87 0x88 0xA3 0xE6   CMOS/NVRAM'e yazıyor — KALICI, geri dönüşü yok
 *   0x51                       dGPU hat kontrolü; Arg2=3 Eject Request
 *   0xC9  (FNKS)               risk defteri "klavye ana şalteri" diyor
 *   0xCA  (PSON)               anlamı bilinmiyor, güç yolu olabilir
 *   0x4B                       PEGP.NLIM + LTGP; EC geri yazıyor (31 Tem 2026)
 *   0xF1 0xF2 0xF3             ECPT güç limitleri; EC geri yazıyor
 *   0x46 0x47 0x50 0x6B 0x70   FDTY/GDTY/FAN1/FAN2 — ÖLÜ YAZMAÇ: bayt tutuyor,
 *                              RPM değişmiyor, hiçbir modda (üç bağımsız kanıt).
 *                              aorus_laptop bunları pwm olarak sunuyor ve YALAN
 *                              söylüyor; biz sunmayacağız.
 *   0x68  (XFNW)               eğri yazma protokolü ölü — XFN1 hiç dolmuyor
 *   0xF6  (KBLL)               ölü yazmaç: yazılıyor, tutuyor, GÖRSEL ETKİSİ YOK
 *                              (7 Eyl 2026, aydınlatma açıkken 3 gidiş-geliş).
 *                              led_classdev sunulmayacak; klavye LampArray işi.
 */

/*
 * ---------------------------------------------------------------------------
 * Fan modu — PECM+0x2C öncelikli seçici
 * ---------------------------------------------------------------------------
 * 0x2C bağımsız bit alanı DEĞİL, bir desen eşleştirici. 16 kombinasyonun tamamı
 * ham WMBD ile denendi (6 Eyl 2026); firmware'in çözümleyicisi de aynı sonucu
 * veriyor (firmware-8051.md §4). Erişilebilir beş desen:
 */
#define AERO_FAN_MODE_DEFAULT	0x00	/* mod 0 — 40 °C'de başlar, tavan %43   */
#define AERO_FAN_MODE_QUIET	0x01	/* sessiz — 54 °C'de başlar, tavan %29  */
#define AERO_FAN_MODE_GAMING	0x02	/* gaming — 40 °C'de başlar, tavan %53  */
#define AERO_FAN_MODE_MODE4	0x09	/* mod 4  — 54 °C'de başlar, tavan %43  */
#define AERO_FAN_MODE_TURBO	0x0C	/* turbo  — 36 °C'de başlar, düz  %63   */

/*
 * Mod 4 (0x09) 7 Eyl 2026'da canlı ölçümle keşfedildi ve makine o sırada zaten
 * ondaydı — aorus_laptop ise fan_mode = 1 diyordu. "Sessiz gibi geç başla,
 * varsayılan gibi yükselebil": günlük kullanım için en dengeli eğri ve hiçbir
 * Linux aracı sunmuyor.
 *
 * Mod yazarken hedef deseni TAM yazmak, diğer üç biti temizlemek şart —
 * dört ayrı seçici (0x57 CRAF, 0x71 FANB, 0x67 TENF, 0x6A ADJF) sırayla
 * çağrılır ve ara durumlar geçici olarak başka bir moda düşürür. Bu yüzden
 * dizinin tamamı aero_ec_lock() altında yürütülmelidir.
 */

/* ------------------------------------------------------------------------- */

/**
 * struct aero_ec - sürücünün paylaşılan durumu
 * @io_lock: WMI çağrılarını sıralar VE aşağıdaki işaretçileri korur.
 *           WMBD/WMBC ikisi de ACPI tarafında Serialized (DSDT 9101/9525),
 *           yani ACPICA tek tek çağrıları zaten kilitliyor. Bu kilit ondan
 *           farklı bir iş yapıyor: (a) çok adımlı dizileri (fan modu = dört
 *           ayrı WMBD çağrısı) bölünmez kılar, (b) remove() sırasında uçuşta
 *           çağrı kalmamasını garanti eder.
 * @wmbd: ABBC0F75 cihazı — yazma yolu. NULL ise sürücü bağlı değil.
 * @wmbc: ABBC0F6F cihazı — okuma yolu.
 * @event: ABBC0F72 cihazı — olay kanalı.
 * @attached: WMBD ve WMBC birlikte hazır; üst katmanlar (hwmon, psy…) kuruldu.
 */
struct aero_ec {
	struct mutex io_lock;
	struct wmi_device *wmbd;
	struct wmi_device *wmbc;
	struct wmi_device *event;
	bool attached;
};

/*
 * EC erişimi. İkisi de io_lock'u KENDİ alır — kilidi tutarken çağırma.
 * Çok adımlı diziler için aero_ec_lock() + __aero_ec_{read,write}() kullan.
 */
int aero_ec_read(u8 selector, u32 arg, u32 *value);
int aero_ec_write(u8 selector, u32 value);

void aero_ec_lock(void);
void aero_ec_unlock(void);
int __aero_ec_read(u8 selector, u32 arg, u32 *value);
int __aero_ec_write(u8 selector, u32 value);

/* hwmon katmanı (aero-hwmon.c) — tamamı salt okunur, yoklama yok */
int aero_hwmon_init(struct device *parent);
void aero_hwmon_exit(void);

/* pil katmanı (aero-battery.c) — şarj limiti, standart power_supply ABI'si */
int aero_battery_init(struct device *parent);
void aero_battery_exit(void);
void aero_battery_resume(void);

/* fan modu (aero-fan.c) — özel sysfs; platform_profile'a GİRMEZ (K1 = a′) */
int __aero_fan_read_pattern(u8 *pattern);	/* çağıran io_lock'u tutmalı */
const char *aero_fan_mode_name(u8 pattern);
extern const struct attribute_group *aero_wmbd_groups[];

#endif /* _AERO_EG61H_H */
