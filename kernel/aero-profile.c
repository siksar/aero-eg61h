// SPDX-License-Identifier: GPL-2.0-only
/*
 * aero_eg61h — platform_profile (ADIM 5)
 *
 * K1 = (a′): bu handler YALNIZ performans profilini (WMBD 0xED, 0-3) anahtarlar.
 * Fan modu pakete GİRMEZ — o kendi sysfs'inde (aero-fan.c).
 *
 * ------------------------------------------------------------------------
 * NEDEN FAN MODU BURADA DEĞİL
 * ------------------------------------------------------------------------
 * Gigabyte Control Center ikisini birlikte anahtarlıyor ve ilk tasarım da
 * öyle öneriyordu. Ama Linux'ta `platform_profile`'ı yazan şey
 * `power-profiles-daemon`, ve o AC/pil değişiminde profili YENİDEN YAZIYOR.
 * Fan modu pakete dahil olsaydı, kullanıcının GUI'den seçtiği mod her fiş
 * takıp çıkarışında sessizce geri alınırdı. Ölçüm de bunları iki ayrı eksen
 * gösteriyor: fan modu PECM+0x2C, profil 0xED.
 *
 * ------------------------------------------------------------------------
 * SEÇİM KÜMESİ amd-pmf'İNKİNİ KAPSAMAK ZORUNDA
 * ------------------------------------------------------------------------
 * Bu makinede `amd-pmf` zaten bir handler kaydetmiş; seçenekleri
 * `low-power balanced performance` (ölçüldü, 7 Eyl). Çekirdek 6.14+ çoklu
 * handler API'sinde `/sys/firmware/acpi/platform_profile`'ın seçenekleri tüm
 * handler'ların KESİŞİMİNE düşüyor.
 *
 * Bizim kümemiz onunkini kapsıyor (+ balanced-performance), dolayısıyla
 * kesişim DARALMIYOR ve `low-power` kaybolmuyor. Bu kritik: `low-power`
 * kaybolsaydı `~/nixos-zixar/system/kernel/power-display.nix`'in pildeki
 * `power-saver` otomatiği bozulur, o da 4.28 W boşta güç bütçesini vururdu.
 *
 * Küme değiştirilecekse bu kısıt önce doğrulanmalı:
 *   cat /sys/firmware/acpi/platform_profile_choices
 *
 * ------------------------------------------------------------------------
 * GERİ OKUMA YOK — VE BU YÜZDEN BAŞLANGIÇ DEĞERİ `custom`
 * ------------------------------------------------------------------------
 * `WMBC`'de 0xED karşılığı YOK (DSDT 9525-9720 tarandı): EC aktif performans
 * profilini geri vermiyor. Sürücünün başka her yazması geri okumayla
 * doğrulanıyor; bu tek istisna ve gizlenmiyor.
 *
 * Sonucu: `profile_get` yalnız BİZİM yazdığımızı hatırlayabiliyor. Modül
 * yüklendiğinde EC'nin hangi profilde olduğunu BİLMİYORUZ, o yüzden başlangıç
 * değeri `PLATFORM_PROFILE_CUSTOM` — ABI'nin "standart profillerden birine
 * karşılık gelmeyen durum" karşılığı. Uydurma bir değer döndürmek
 * (`balanced` demek gibi) the legacy driver's behaviornın aynısı olurdu.
 *
 * Probe'ta bir profil YAZMIYORUZ: kullanıcının dokunmadığı bir ayarı sürücü
 * zorlamaz.
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/bitops.h>
#include <linux/device.h>
#include <linux/err.h>
#include <linux/errno.h>
#include <linux/kernel.h>
#include <linux/platform_profile.h>

#include "aero-eg61h.h"

static struct device *aero_pprof_dev;

/* Bizim yazdığımız son profil. CUSTOM = "hiç yazmadık, EC'nin durumu bilinmiyor". */
static enum platform_profile_option aero_pprof_cur = PLATFORM_PROFILE_CUSTOM;

/*
 * SEÇİM KÜMESİ `amd-pmf`'İNKİYLE **BİREBİR AYNI** — bu ölçümle seçildi.
 *
 * Önce `balanced-performance` de sunuluyordu (0xED 3). Ölçüm (7 Eyl) iki şey
 * gösterdi:
 *
 *   1. Legacy düğüm (`/sys/firmware/acpi/platform_profile_choices`) BİZİM
 *      kümemizi gösteriyor. Kümemizden `low-power`'ı çıkarınca legacy'den de
 *      kayboldu — yani üst-küme kısıtı gerçek, ama mekanizma "kesişim" değil.
 *   2. Fazladan sunduğumuz `balanced-performance` legacy'ye çıkıyor ve oradan
 *      yazıldığında `amd-pmf`'E DE GİDİYOR — kendi `choices`'inde olmamasına
 *      rağmen `profile`'ı `balanced-performance` okudu. SMU tarafında ne
 *      yaptığı ÖLÇÜLMEDİ.
 *
 * Ölçülmemiş bir yan etki için fazladan bir seçenek koymuyoruz. Küme artık
 * `amd-pmf`'inkiyle aynı, dolayısıyla legacy düğüm modül yüklenmeden önceki
 * hâliyle BİREBİR aynı kalıyor — "hiçbir şeyi bozmadık"ın en güçlü hâli.
 *
 * Bedeli: `0xED 3` `platform_profile` üzerinden erişilemiyor. Kimse
 * kullanmıyordu (`sched.nix` oyunda `0xED 2` yazıyor) ve 0/1/2 düşük/dengeli/
 * performans aralığını zaten kapsıyor.
 *
 * profil ↔ WMBD 0xED eşlemesi.
 *
 * KAYNAK: DSDT 9200-9336 (0xED'in dört dalı). Bu STATİK ÇÖZÜMLEME, canlı güç
 * ölçümü DEĞİL — ASL'nin her modda hangi alanlara ne yazdığı okundu:
 *
 *   0xED  ATPP        ACBT          AC PL1/PL2/PL3   DC PL1/PL2/PL3
 *   ----  ----------  ------------  ---------------  --------------
 *    0    0xA0        0 (kapalı)    20/65/65 W       15/30/30 W
 *    1    0xC8        0x50          25/65/80 W       20/54/54 W
 *    2    ECPL'e göre 0xA0 (en üst) 30/80/80 W       20/54/54 W
 *    3    0xC8        0xA0          25/80/80 W       20/54/54 W   ← sunulmuyor
 *
 * Sıralama AC PL1'e göre: 0 (20W) < 1 (25W) < 2 (30W).
 * `~/nixos-zixar/system/kernel/sched.nix`'in oyunda `0xED 2` yazması bununla
 * tutarlı.
 *
 * AÇIK İŞ: bunu canlı güç ölçümüyle doğrula (yük altında paket gücü + dGPU
 * bütçesi, üç profilde). ASL doğru okundu ama "ASL ne yazıyor" ile "donanım
 * ne yapıyor" aynı şey değil — bu deponun tekrar tekrar öğrendiği ders.
 */
static const struct {
	enum platform_profile_option profile;
	u8 value;
} aero_pprof_map[] = {
	{ PLATFORM_PROFILE_LOW_POWER,   0 },
	{ PLATFORM_PROFILE_BALANCED,    1 },
	{ PLATFORM_PROFILE_PERFORMANCE, 2 },
};

static int aero_pprof_probe_choices(void *drvdata, unsigned long *choices)
{
	size_t i;

	for (i = 0; i < ARRAY_SIZE(aero_pprof_map); i++)
		set_bit(aero_pprof_map[i].profile, choices);

	return 0;
}

static int aero_pprof_get(struct device *dev,
			  enum platform_profile_option *profile)
{
	*profile = aero_pprof_cur;
	return 0;
}

static int aero_pprof_set(struct device *dev,
			  enum platform_profile_option profile)
{
	size_t i;
	int ret;

	for (i = 0; i < ARRAY_SIZE(aero_pprof_map); i++) {
		if (aero_pprof_map[i].profile != profile)
			continue;

		ret = aero_ec_write(AERO_WR_PERF_PROFILE,
				    aero_pprof_map[i].value);
		if (ret)
			return ret;

		/*
		 * Geri okuyup doğrulayamıyoruz (WMBC'de 0xED yok), o yüzden
		 * yalnız ACPI değerlendirmesinin başarısına bakıyoruz. Bu,
		 * sürücünün tek doğrulanamayan yazımı — bilerek ve belgeli.
		 */
		aero_pprof_cur = profile;
		return 0;
	}

	return -EOPNOTSUPP;
}

static const struct platform_profile_ops aero_pprof_ops = {
	.probe       = aero_pprof_probe_choices,
	.profile_get = aero_pprof_get,
	.profile_set = aero_pprof_set,
};

int aero_profile_init(struct device *parent)
{
	aero_pprof_dev = platform_profile_register(parent, "aero_eg61h", NULL,
						   &aero_pprof_ops);
	if (IS_ERR(aero_pprof_dev)) {
		int ret = PTR_ERR(aero_pprof_dev);

		aero_pprof_dev = NULL;
		return ret;
	}

	return 0;
}

void aero_profile_exit(void)
{
	if (!aero_pprof_dev)
		return;

	platform_profile_remove(aero_pprof_dev);
	aero_pprof_dev = NULL;
	aero_pprof_cur = PLATFORM_PROFILE_CUSTOM;
}
