// SPDX-License-Identifier: GPL-2.0-only
/*
 * aero_eg61h — dGPU Dynamic Boost bütçesi
 *
 *   /sys/bus/wmi/devices/ABBC0F75-8EA1-11D1-00A0-C90629100000-2/dgpu_boost
 *
 * WMBD 0x4C, NPCF.ACBT = değer × 8 W, 0-10. Bu yazım önceden NixOS modülünde
 * `acpi_call` üzerinden ham `\_SB.PCI0.AMW0.WMBD 0 0x4C N` olarak yapılıyordu:
 * io_lock'u, DMI kapısını ve çakışma kapısını atlıyordu, üstelik `acpi_call`
 * yüklü kaldığı sürece root herhangi bir ACPI metodunu çağırabiliyordu.
 * Artık sürücünün diğer yazımlarıyla aynı kilit altında sıralanıyor.
 *
 * GERİ OKUMA YOK: WMBC'de 0x4C karşılığı bilinmiyor. aero-profile.c'deki
 * 0xED gibi, `show` yalnız BİZİM yazdığımız son değeri döndürür; hiç
 * yazılmadıysa `unknown`. Uydurma bir başlangıç değeri göstermiyoruz.
 *
 * NOT: 0xED (platform_profile) da ACBT'yi yazıyor. Profil değiştiğinde bu
 * bütçe EC tarafında değişmiş olabilir; `show` bunu bilemez.
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/device.h>
#include <linux/errno.h>
#include <linux/kernel.h>
#include <linux/sysfs.h>

#include "aero-eg61h.h"

/* Bizim yazdığımız son değer. -1 = "hiç yazmadık, EC'nin durumu bilinmiyor". */
static int aero_dgpu_boost = -1;

static ssize_t dgpu_boost_show(struct device *dev,
			       struct device_attribute *attr, char *buf)
{
	int val;

	aero_ec_lock();
	val = aero_dgpu_boost;
	aero_ec_unlock();

	if (val < 0)
		return sysfs_emit(buf, "unknown\n");

	return sysfs_emit(buf, "%d\n", val);
}

static ssize_t dgpu_boost_store(struct device *dev,
				struct device_attribute *attr,
				const char *buf, size_t count)
{
	unsigned int val;
	int ret;

	ret = kstrtouint(buf, 0, &val);
	if (ret)
		return ret;

	if (val > AERO_DGPU_BOOST_MAX)
		return -EINVAL;

	aero_ec_lock();
	ret = __aero_ec_write(AERO_WR_DGPU_BOOST, val);
	if (!ret)
		aero_dgpu_boost = val;
	aero_ec_unlock();

	return ret ? ret : count;
}

static DEVICE_ATTR_RW(dgpu_boost);

static struct attribute *aero_gpu_attrs[] = {
	&dev_attr_dgpu_boost.attr,
	NULL
};

const struct attribute_group aero_gpu_group = {
	.attrs = aero_gpu_attrs,
};
