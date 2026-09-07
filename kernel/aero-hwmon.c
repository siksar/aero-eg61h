// SPDX-License-Identifier: GPL-2.0-only
/*
 * aero_eg61h — hwmon katmanı (ADIM 2)
 *
 * TAMAMI SALT OKUNUR. Üç kanal, üçü de ölçülmüş:
 *
 *   temp1  CPU sıcaklığı   WMBC 0xE1  (CTMP, ECMM+0xB0)  °C
 *   fan1   fan 0 devri     WMBC 0xE4  (RPM1, PECM+0x13)  16 bit
 *   fan2   fan 1 devri     WMBC 0xE5  (RPM2, PECM+0x15)  16 bit
 *
 * Burada OLMAYAN üç şeyin gerekçesi (hepsi "ölçülmemiş yeteneği sunma"
 * kuralının doğrudan sonucu):
 *
 * 1. `pwm*` YOK — ne yazılabilir ne salt okunur. Yazılabilir olamaz: FDTY/GDTY/
 *    FAN1/FAN2 baytları yazımı kabul ediyor, bayt tutuyor, RPM DEĞİŞMİYOR —
 *    hiçbir modda, üç bağımsız kanıtla. Salt okunur da olamaz: o baytlar gerçek
 *    duty'yi göstermiyor (gerçek duty EC XRAM 0xF8D6/0xF90C'de, host penceresinin
 *    kapalı kısmında). aorus_laptop'ın pwm1/pwm2 sunması bir hatadır.
 *
 * 2. `temp2` (SKTC, WMBC 0xE2/0xE3) YOK — ÖLÇÜLDÜ, ÖLÜ KANAL. 7 Eyl 2026'da
 *    iki bağımsız koşu: boştayken 0 (aorus_laptop'ın temp2_input'u da aynı
 *    anda 0), ve TAM YÜK altında — CPU 91 °C, fanlar 3333/3703 rpm — hâlâ 0.
 *    Okuma doğru, alan boş. Bu kanal bir daha açılmayacak.
 *    aorus_laptop temp2 VE temp3 sunuyor, ikisi de sıfır: aynı hata sınıfı.
 *
 * 3. Dört fan YOK, iki fan var — donanımda iki fan ölçüldü, firmware'de iki
 *    kayıtlı fan dizisi var. aorus_laptop'ın fan3/fan4 iddiası doğrulanmadı.
 *
 * ETİKETLER: firmware fanları "fan 0" / "fan 1" diye adlandırıyor ve hiçbir
 * yerde CPU/GPU ataması yok (ne DSDT'de ne disassembly'de). Bu yüzden etiketler
 * "Fan 1"/"Fan 2" — tasarım belgesindeki "CPU Fan"/"GPU Fan" ölçülmemiş bir
 * tahmindi. Hangi fanın neyi soğuttuğu ölçülürse etiket güncellenir.
 *
 * YOKLAMA YOK: bu dosya kendiliğinden hiçbir şey okumaz. Her okuma, userspace
 * bir hwmon dosyasını okuduğunda olur (okuma başına bir ACPI/WMI çağrısı).
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/device.h>
#include <linux/err.h>
#include <linux/errno.h>
#include <linux/hwmon.h>
#include <linux/kernel.h>

#include "aero-eg61h.h"

static struct device *aero_hwmon_dev;

static const char *const aero_temp_labels[] = { "CPU" };
static const char *const aero_fan_labels[]  = { "Fan 1", "Fan 2" };

static umode_t aero_hwmon_is_visible(const void *data,
				     enum hwmon_sensor_types type,
				     u32 attr, int channel)
{
	/* Tek bir yazılabilir öznitelik bile yok — bu kasıtlı, yukarıdaki (1). */
	return 0444;
}

static int aero_hwmon_read(struct device *dev, enum hwmon_sensor_types type,
			   u32 attr, int channel, long *val)
{
	u32 raw;
	int ret;

	switch (type) {
	case hwmon_temp:
		if (attr != hwmon_temp_input || channel != 0)
			return -EOPNOTSUPP;

		ret = aero_ec_read(AERO_RD_CPU_TEMP, 0, &raw);
		if (ret)
			return ret;

		/* CTMP 8 bit, °C. hwmon millidereceyi ister. */
		*val = (long)raw * 1000;
		return 0;

	case hwmon_fan:
		if (attr != hwmon_fan_input)
			return -EOPNOTSUPP;

		switch (channel) {
		case 0:
			ret = aero_ec_read(AERO_RD_FAN1_RPM, 0, &raw);
			break;
		case 1:
			ret = aero_ec_read(AERO_RD_FAN2_RPM, 0, &raw);
			break;
		default:
			return -EOPNOTSUPP;
		}
		if (ret)
			return ret;

		/*
		 * 0 geçerli bir cevap: sessiz/mod4 eğrilerinde fanlar eşiğin
		 * altında gerçekten duruyor (ölçüldü). Hata olarak bildirmiyoruz.
		 */
		*val = raw;
		return 0;

	default:
		return -EOPNOTSUPP;
	}
}

static int aero_hwmon_read_string(struct device *dev,
				  enum hwmon_sensor_types type,
				  u32 attr, int channel, const char **str)
{
	switch (type) {
	case hwmon_temp:
		if (attr != hwmon_temp_label ||
		    channel >= ARRAY_SIZE(aero_temp_labels))
			return -EOPNOTSUPP;
		*str = aero_temp_labels[channel];
		return 0;

	case hwmon_fan:
		if (attr != hwmon_fan_label ||
		    channel >= ARRAY_SIZE(aero_fan_labels))
			return -EOPNOTSUPP;
		*str = aero_fan_labels[channel];
		return 0;

	default:
		return -EOPNOTSUPP;
	}
}

static const struct hwmon_ops aero_hwmon_ops = {
	.is_visible  = aero_hwmon_is_visible,
	.read        = aero_hwmon_read,
	.read_string = aero_hwmon_read_string,
};

static const struct hwmon_channel_info *const aero_hwmon_info[] = {
	HWMON_CHANNEL_INFO(temp,
			   HWMON_T_INPUT | HWMON_T_LABEL),		/* CPU  */
	HWMON_CHANNEL_INFO(fan,
			   HWMON_F_INPUT | HWMON_F_LABEL,		/* fan 0 */
			   HWMON_F_INPUT | HWMON_F_LABEL),		/* fan 1 */
	NULL
};

static const struct hwmon_chip_info aero_hwmon_chip_info = {
	.ops  = &aero_hwmon_ops,
	.info = aero_hwmon_info,
};

int aero_hwmon_init(struct device *parent)
{
	/*
	 * devm_ KULLANILMIYOR. devm ömrü ebeveyn cihaza bağlanır (WMBD), oysa
	 * bu katman "WMBD ve WMBC ikisi birden bağlı" koşuluna bağlı ve WMBC
	 * ayrılınca da sökülmesi gerekiyor. Sökümü açıkça yapıyoruz.
	 */
	aero_hwmon_dev = hwmon_device_register_with_info(parent, "aero_eg61h",
							 NULL,
							 &aero_hwmon_chip_info,
							 NULL);
	if (IS_ERR(aero_hwmon_dev)) {
		int ret = PTR_ERR(aero_hwmon_dev);

		aero_hwmon_dev = NULL;
		return ret;
	}

	return 0;
}

void aero_hwmon_exit(void)
{
	if (!aero_hwmon_dev)
		return;

	hwmon_device_unregister(aero_hwmon_dev);
	aero_hwmon_dev = NULL;
}
