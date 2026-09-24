// SPDX-License-Identifier: GPL-2.0-only
/*
 * aero_eg61h — fan modu (ADIM 4)
 *
 *   /sys/bus/wmi/devices/ABBC0F75-8EA1-11D1-00A0-C90629100000-2/fan_mode
 *   /sys/bus/wmi/devices/ABBC0F75-.../fan_mode_choices     (salt okunur)
 *
 * ------------------------------------------------------------------------
 * `PECM+0x2C` BAĞIMSIZ BİT ALANI DEĞİL, BİR DESEN EŞLEŞTİRİCİ
 * ------------------------------------------------------------------------
 * 16 kombinasyonun tamamı ham WMBD ile denendi (6 Eyl 2026) ve firmware'in
 * çözümleyicisi de aynı sonucu veriyor (firmware-8051.md §4): yalnız beş desen
 * tanınıyor, kalan on biri varsayılana düşüyor. Yani "bit çevir" diye bir şey
 * yok — hedef desen TAM yazılmalı, kalan bitler temizlenmeli.
 *
 * Deseni yazan tek bir seçici de yok: dört ayrı WMBD seçicisi dört ayrı biti
 * yazıyor (0x57 CRAF bit0, 0x71 FANB bit1, 0x67 TENF bit2, 0x6A ADJF bit3).
 * Dört yazım arasındaki ara durumlar geçici olarak BAŞKA bir moda düşürüyor,
 * o yüzden:
 *
 *   - dizinin tamamı tek bir aero_ec_lock() altında yürüyor
 *   - önce temizlenecek bitler yazılıyor, sonra kurulacaklar. Sebebi: eksik
 *     bitli ara desenler varsayılana (mod 0 — 40 °C'de başlar, %43 tavan)
 *     düşüyor, yani ara durum her zaman DAHA SOĞUK bir moda denk geliyor.
 *     Ters sırada ara durum turbo'ya yakın desenlere düşebilirdi.
 *
 * WMBD'nin dönüşü bilgi taşımadığı için (ölçüldü) dizinin sonunda desen
 * DÖRT SEÇİCİYLE GERİ OKUNUP karşılaştırılıyor. the legacy driver's failure to
 * `fan_mode` bildirmesinin sebebi tam olarak bu doğrulamayı yapmaması.
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/bitops.h>
#include <linux/device.h>
#include <linux/errno.h>
#include <linux/kernel.h>
#include <linux/string.h>
#include <linux/sysfs.h>
#include <linux/wmi.h>

#include "aero-eg61h.h"

/*
 * Beş erişilebilir mod. İsimler PLAN.md §5'in ön ayar isimleriyle tutarlı.
 * Eğri özetleri firmware'den çıkarıldı (24 tablo + 0x0616E kural tablosu) ve
 * sekizi canlı ölçümle birebir doğrulandı.
 */
static const struct aero_fan_mode {
	const char *name;
	u8 pattern;
	const char *desc;
} aero_fan_modes[] = {
	{ "quiet",      AERO_FAN_MODE_QUIET,   "54 °C'de başlar, tavan %29" },
	{ "balanced",   AERO_FAN_MODE_MODE4,   "54 °C'de başlar, tavan %43" },
	{ "responsive", AERO_FAN_MODE_DEFAULT, "40 °C'de başlar, tavan %43" },
	{ "gaming",     AERO_FAN_MODE_GAMING,  "40 °C'de başlar, tavan %53" },
	{ "turbo",      AERO_FAN_MODE_TURBO,   "36 °C'de başlar, düz %63"   },
};

/* Desen bitleri ↔ WMBD seçicileri. Sıra bu dizide değil, aşağıdaki iki geçişte. */
static const struct {
	u8 selector;
	u8 bit;
} aero_fan_bits[] = {
	{ AERO_WR_FAN_CRAF, 0 },
	{ AERO_WR_FAN_FANB, 1 },
	{ AERO_WR_FAN_TENF, 2 },
	{ AERO_WR_FAN_ADJF, 3 },
};

/*
 * 0x2C tek bir WMBC seçicisiyle bütün olarak okunamıyor — dört bit dört ayrı
 * seçicide. Çağıran io_lock'u tutmalı.
 */
int __aero_fan_read_pattern(u8 *pattern)
{
	static const u8 sel[4] = {
		AERO_RD_FAN_CRAF, AERO_RD_FAN_FANB,
		AERO_RD_FAN_TENF, AERO_RD_FAN_ADJF,
	};
	u8 val = 0;
	int i, ret;

	for (i = 0; i < 4; i++) {
		u32 bit;

		ret = __aero_ec_read(sel[i], 0, &bit);
		if (ret)
			return ret;

		val |= (bit & 1) << i;
	}

	*pattern = val;
	return 0;
}

const char *aero_fan_mode_name(u8 pattern)
{
	size_t i;

	for (i = 0; i < ARRAY_SIZE(aero_fan_modes); i++)
		if (aero_fan_modes[i].pattern == pattern)
			return aero_fan_modes[i].name;

	return "unknown";
}

static int aero_fan_write_pattern(u8 pattern)
{
	u8 back = 0xff;
	size_t i;
	int ret;

	aero_ec_lock();

	/* 1. geçiş: temizlenecek bitler. Ara desenler varsayılana (daha soğuk) düşer. */
	for (i = 0; i < ARRAY_SIZE(aero_fan_bits); i++) {
		if (pattern & BIT(aero_fan_bits[i].bit))
			continue;

		ret = __aero_ec_write(aero_fan_bits[i].selector, 0);
		if (ret)
			goto out;
	}

	/* 2. geçiş: kurulacak bitler. */
	for (i = 0; i < ARRAY_SIZE(aero_fan_bits); i++) {
		if (!(pattern & BIT(aero_fan_bits[i].bit)))
			continue;

		ret = __aero_ec_write(aero_fan_bits[i].selector, 1);
		if (ret)
			goto out;
	}

	/* WMBD'nin dönüşü bilgi taşımıyor — tek doğrulama yolu geri okumak. */
	ret = __aero_fan_read_pattern(&back);
out:
	aero_ec_unlock();

	if (ret)
		return ret;

	if (back != pattern) {
		pr_warn("fan mode 0x%02x was written but EC reads 0x%02x — write did not stick\n",
			pattern, back);
		return -EIO;
	}

	return 0;
}

static ssize_t fan_mode_show(struct device *dev, struct device_attribute *attr,
			     char *buf)
{
	u8 pattern;
	int ret;

	aero_ec_lock();
	ret = __aero_fan_read_pattern(&pattern);
	aero_ec_unlock();

	if (ret)
		return ret;

	if (!strcmp(aero_fan_mode_name(pattern), "unknown")) {
		/*
		 * Olmaması gereken durum: EC tanınmayan bir desende. Sayıyı
		 * gizlemiyoruz — "unknown" deyip ham deseni loglamak, yanlış bir
		 * isim uydurmaktan iyi.
		 */
		dev_warn(dev, "0x2C has an unknown pattern: 0x%02x\n", pattern);
		return sysfs_emit(buf, "unknown\n");
	}

	return sysfs_emit(buf, "%s\n", aero_fan_mode_name(pattern));
}

static ssize_t fan_mode_store(struct device *dev, struct device_attribute *attr,
			      const char *buf, size_t count)
{
	size_t i;
	int ret;

	for (i = 0; i < ARRAY_SIZE(aero_fan_modes); i++) {
		if (sysfs_streq(buf, aero_fan_modes[i].name)) {
			ret = aero_fan_write_pattern(aero_fan_modes[i].pattern);
			return ret ? ret : count;
		}
	}

	return -EINVAL;
}

static ssize_t fan_mode_choices_show(struct device *dev,
				     struct device_attribute *attr, char *buf)
{
	ssize_t len = 0;
	size_t i;

	for (i = 0; i < ARRAY_SIZE(aero_fan_modes); i++)
		len += sysfs_emit_at(buf, len, "%s%s",
				     i ? " " : "", aero_fan_modes[i].name);

	return len + sysfs_emit_at(buf, len, "\n");
}

static DEVICE_ATTR_RW(fan_mode);
static DEVICE_ATTR_RO(fan_mode_choices);

static struct attribute *aero_fan_attrs[] = {
	&dev_attr_fan_mode.attr,
	&dev_attr_fan_mode_choices.attr,
	NULL
};

static const struct attribute_group aero_fan_group = {
	.attrs = aero_fan_attrs,
};

const struct attribute_group *aero_wmbd_groups[] = {
	&aero_fan_group,
	&aero_gpu_group,
	NULL
};
