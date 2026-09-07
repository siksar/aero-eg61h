// SPDX-License-Identifier: GPL-2.0-only
/*
 * aero_eg61h — Gigabyte AERO X16 1VH (SKU EG61VH) platform sürücüsü
 *
 * ADIM 1: iskelet. WMI bus'a gerçek bağlanma + WMBC/WMBD sarmalayıcıları.
 * Bu adımda hiçbir sysfs düğümü, hwmon kanalı ya da yazma yolu YOK.
 * Doğrulama ölçütü: /sys/bus/wmi/drivers/aero_eg61h görünür, dmesg temiz.
 *
 * Neden yeni bir sürücü? Bu makinede üreticinin Linux sürücüsü yok; elde olan
 * topluluk sürücüsü (aorus_laptop) genel bir Gigabyte sürücüsü ve ÇALIŞMAYAN
 * kontroller sunuyor: yazılabilir pwm1/pwm2 düğümleri var, fan onları
 * umursamıyor (ölçüldü, üç bağımsız kanıt). Ayrıca yanlış bildiriyor —
 * 7 Eyl 2026'da fan_mode = 1 derken makine mod 4'te (PECM+0x2C = 0x09)
 * koşuyordu. Bu sürücünün değişmez kuralı: ölçülmemiş hiçbir yeteneği sunma.
 *
 * Copyright (c) 2026 zixar
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/acpi.h>
#include <linux/device.h>
#include <linux/dmi.h>
#include <linux/errno.h>
#include <linux/kernel.h>
#include <linux/module.h>
#include <linux/mutex.h>
#include <linux/platform_device.h>
#include <linux/slab.h>
#include <linux/wmi.h>

#include "aero-eg61h.h"

static bool force;
module_param(force, bool, 0444);
MODULE_PARM_DESC(force,
	"DMI eslesmesi olmasa ya da aorus_laptop yuklu olsa da baglan (varsayilan: hayir)");

static struct aero_ec aero = {
	.io_lock = __MUTEX_INITIALIZER(aero.io_lock),
};

/*
 * Bu sürücü DSDT/EC sürümüne bağlı, elle tersine mühendislik edilmiş seçici
 * değerleri kullanıyor. GUID'ler Gigabyte genelinde ortak olduğu için başka bir
 * modelde de eşleşirler — ama seçicilerin anlamı eşleşmez. DMI kapısı bu yüzden
 * var: yanlış makinede bağlanmak, ölçülmemiş bir baytı ölçülmüş sanmak demek.
 */
static const struct dmi_system_id aero_dmi_table[] = {
	{
		.ident = "Gigabyte AERO X16 1VH (EG61VH)",
		.matches = {
			DMI_MATCH(DMI_SYS_VENDOR, "GIGABYTE"),
			DMI_MATCH(DMI_PRODUCT_SKU, "EG61VH"),
		},
	},
	{ }
};
MODULE_DEVICE_TABLE(dmi, aero_dmi_table);

/* ------------------------------------------------------------------------- *
 * EC erişimi
 * ------------------------------------------------------------------------- */

/*
 * WMI çekirdeği Arg2'yi ACPI_TYPE_BUFFER olarak geçiriyor. WMBD/WMBC dalları
 * Arg2'yi tamsayı gibi kullanıyor (CRAF = Arg2 gibi); ACPI'nin örtük
 * Buffer->Integer dönüşümü tamponun ilk 8 baytını küçük-endian okuyor, yani
 * 4 baytlık bir u32 doğru değeri veriyor. (Tek istisna 0x63: DerefOf ile
 * gerçekten indeksliyor — o seçici YASAK sınıfında, hiç çağrılmayacak.)
 */
static int aero_wmi_eval(struct wmi_device *wdev, u8 selector, u32 arg,
			 union acpi_object **result)
{
	struct acpi_buffer in = { sizeof(arg), &arg };
	struct acpi_buffer out = { ACPI_ALLOCATE_BUFFER, NULL };
	acpi_status status;

	status = wmidev_evaluate_method(wdev, 0x0, selector, &in, &out);
	if (ACPI_FAILURE(status)) {
		kfree(out.pointer);
		return -EIO;
	}

	/* NULL geçerli bir sonuç: açık Return'ü olmayan dallar hiçbir şey döndürmez */
	*result = out.pointer;
	return 0;
}

int __aero_ec_read(u8 selector, u32 arg, u32 *value)
{
	union acpi_object *obj = NULL;
	int ret;

	lockdep_assert_held(&aero.io_lock);

	if (!aero.wmbc)
		return -ENODEV;

	ret = aero_wmi_eval(aero.wmbc, selector, arg, &obj);
	if (ret)
		return ret;

	if (!obj)
		return -ENODATA;

	/*
	 * Tanınmayan seçici Arg2'yi AYNEN yankılıyor (DSDT 9712-9715), yani
	 * dönüş tipi Buffer olur. Bunu tamsayı sanmak, mevcut sürücünün
	 * yetenek-tespiti zincirinin kökündeki hata. Tip denetimi bu yüzden
	 * sıkı: Integer değilse okuma geçersizdir.
	 */
	if (obj->type != ACPI_TYPE_INTEGER) {
		pr_debug("WMBC 0x%02x tamsayi degil (tip %u) — taninmayan secici?\n",
			 selector, obj->type);
		ret = -EPROTO;
		goto out;
	}

	*value = (u32)obj->integer.value;
out:
	kfree(obj);
	return ret;
}

int __aero_ec_write(u8 selector, u32 value)
{
	union acpi_object *obj = NULL;
	int ret;

	lockdep_assert_held(&aero.io_lock);

	if (!aero.wmbd)
		return -ENODEV;

	ret = aero_wmi_eval(aero.wmbd, selector, value, &obj);

	/*
	 * DÖNÜŞ DEĞERİ HİÇBİR BİLGİ TAŞIMIYOR — ölçüldü (6 Eyl 2026, 0xC7 ile):
	 * açık Return'ü olmayan bir dal bile metot sonundaki Return(Arg2)'ye
	 * düşüp girdiyi yankılıyor, yani "tanindi" ile "tanınmadı" ayırt
	 * edilemiyor. Bu yüzden sonuca BAKMIYORUZ; yalnız ACPI değerlendirmesinin
	 * kendisi başarılı mı, onu bildiriyoruz. Bir yazımın tuttuğunu görmenin
	 * tek yolu karşılık gelen WMBC seçicisiyle geri okumaktır.
	 */
	kfree(obj);
	return ret;
}

int aero_ec_read(u8 selector, u32 arg, u32 *value)
{
	int ret;

	mutex_lock(&aero.io_lock);
	ret = __aero_ec_read(selector, arg, value);
	mutex_unlock(&aero.io_lock);

	return ret;
}

int aero_ec_write(u8 selector, u32 value)
{
	int ret;

	mutex_lock(&aero.io_lock);
	ret = __aero_ec_write(selector, value);
	mutex_unlock(&aero.io_lock);

	return ret;
}

void aero_ec_lock(void)
{
	mutex_lock(&aero.io_lock);
}

void aero_ec_unlock(void)
{
	mutex_unlock(&aero.io_lock);
}

/* ------------------------------------------------------------------------- *
 * Çekirdek katman — WMBD ve WMBC birlikte hazır olduğunda kurulur
 * ------------------------------------------------------------------------- */

/*
 * PECM+0x2C'nin dört bitini ayrı ayrı okuyup deseni geri kuruyor.
 * 0x2C tek bir WMBC seçicisiyle bütün olarak okunamıyor; dört bit dört ayrı
 * seçicide (0x57 CRAF bit0, 0x71 FANB bit1, 0x67 TENF bit2, 0x6A ADJF bit3).
 */
static int aero_read_fan_pattern(u8 *pattern)
{
	static const u8 sel[4] = {
		AERO_RD_FAN_CRAF, AERO_RD_FAN_FANB,
		AERO_RD_FAN_TENF, AERO_RD_FAN_ADJF,
	};
	u8 val = 0;
	int i, ret;

	lockdep_assert_held(&aero.io_lock);

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

static const char *aero_fan_mode_name(u8 pattern)
{
	switch (pattern) {
	case AERO_FAN_MODE_DEFAULT:	return "mod0/varsayilan";
	case AERO_FAN_MODE_QUIET:	return "sessiz";
	case AERO_FAN_MODE_GAMING:	return "gaming";
	case AERO_FAN_MODE_MODE4:	return "mod4";
	case AERO_FAN_MODE_TURBO:	return "turbo";
	default:			return "tanimsiz-desen";
	}
}

/*
 * Bağlanma kanıtı. Adım 1'in tek çıktısı bu: okuma yolunun gerçekten çalıştığını
 * tek seferlik bir okumayla gösterir. Yoklama DEĞİL — bir kez, probe anında.
 * (Boşta güç bütçesi kuralı: bu sürücü boştayken hiçbir şey okumaz.)
 */
static void aero_core_attach(void)
{
	u32 cpu = 0, soc = 0, rpm1 = 0, rpm2 = 0;
	u8 pattern = 0xff;
	int ret;

	aero_ec_lock();
	ret = __aero_ec_read(AERO_RD_CPU_TEMP, 0, &cpu);
	if (!ret)
		ret = __aero_ec_read(AERO_RD_SOC_TEMP, 0, &soc);
	if (!ret)
		ret = __aero_ec_read(AERO_RD_FAN1_RPM, 0, &rpm1);
	if (!ret)
		ret = __aero_ec_read(AERO_RD_FAN2_RPM, 0, &rpm2);
	if (!ret)
		ret = aero_read_fan_pattern(&pattern);
	aero_ec_unlock();

	if (ret) {
		pr_warn("EC okuma yolu calismiyor (%d) — WMBC yaniti beklenmedik\n",
			ret);
		return;
	}

	pr_info("EC okuma yolu calisiyor: CPU %u C, soket %u C, fan %u/%u rpm\n",
		cpu, soc, rpm1, rpm2);
	pr_info("fan modu: 0x%02x (%s)\n", pattern, aero_fan_mode_name(pattern));
}

static void aero_core_detach(void)
{
	/* Adım 1'de sökülecek bir şey yok; hwmon/psy/platform_profile buraya gelecek. */
}

/*
 * probe/remove ortak kuyruğu. Kilit yalnız işaretçileri ve @attached'i korumak
 * için tutulur; attach/detach işi kilit DIŞINDA yapılır, çünkü orası WMI
 * çağrısı yapıyor ve io_lock'u kendi almak zorunda.
 */
static void aero_device_bound(struct wmi_device **slot, struct wmi_device *wdev)
{
	bool attach_now;

	mutex_lock(&aero.io_lock);
	*slot = wdev;
	attach_now = aero.wmbd && aero.wmbc && !aero.attached;
	if (attach_now)
		aero.attached = true;
	mutex_unlock(&aero.io_lock);

	if (attach_now)
		aero_core_attach();
}

static void aero_device_unbound(struct wmi_device **slot)
{
	bool detach_now;

	mutex_lock(&aero.io_lock);
	detach_now = aero.attached;
	aero.attached = false;
	mutex_unlock(&aero.io_lock);

	if (detach_now)
		aero_core_detach();

	/*
	 * İşaretçiyi io_lock altında sıfırlamak, uçuşta bir WMI çağrısı
	 * kalmadığını garanti eder: her çağrı aynı kilidi tutuyor.
	 */
	mutex_lock(&aero.io_lock);
	*slot = NULL;
	mutex_unlock(&aero.io_lock);
}

/* ------------------------------------------------------------------------- *
 * Çakışma kapısı
 * ------------------------------------------------------------------------- */

/*
 * aorus_laptop WMI bus'a bağlanmıyor — kendi platform cihazını kurup WMI
 * metotlarını GUID üzerinden doğrudan çağırıyor. Yani bizim bus'a bağlanmamız
 * onu engellemiyor; ikisi aynı anda yüklüyken aynı WMBD metodunu çağırırlar ve
 * özellikle fan modu yazımı sırasında (dört ayrı çağrılık dizi) yarış üretir.
 * Sessizce yan yana koşmak yerine bağlanmayı reddediyoruz.
 */
static bool aero_conflicting_driver_present(void)
{
	struct device *dev;

	dev = bus_find_device_by_name(&platform_bus_type, NULL, "aorus_laptop");
	if (!dev)
		return false;

	put_device(dev);
	return true;
}

static int aero_check_platform(struct wmi_device *wdev)
{
	if (!dmi_check_system(aero_dmi_table)) {
		if (!force) {
			dev_info(&wdev->dev,
				 "bu makine EG61VH degil — baglanmiyorum (force=1 ile zorlanabilir)\n");
			return -ENODEV;
		}
		dev_warn(&wdev->dev,
			 "force=1: DMI eslesmedi, secici anlamlari DOGRULANMAMIS\n");
	}

	if (aero_conflicting_driver_present()) {
		if (!force) {
			dev_err(&wdev->dev,
				"aorus_laptop yuklu — ikisi ayni WMI metotlarini cagiriyor, yaris uretir.\n");
			dev_err(&wdev->dev,
				"once onu kaldirin:  sudo rmmod aorus_laptop\n");
			return -EBUSY;
		}
		dev_warn(&wdev->dev,
			 "force=1: aorus_laptop yuklu, fan modu yazimlari yarisabilir\n");
	}

	return 0;
}

/* ------------------------------------------------------------------------- *
 * WMI sürücüleri
 * ------------------------------------------------------------------------- */

static int aero_wmbd_probe(struct wmi_device *wdev, const void *context)
{
	int ret;

	ret = aero_check_platform(wdev);
	if (ret)
		return ret;

	aero_device_bound(&aero.wmbd, wdev);
	return 0;
}

static void aero_wmbd_remove(struct wmi_device *wdev)
{
	aero_device_unbound(&aero.wmbd);
}

static int aero_wmbc_probe(struct wmi_device *wdev, const void *context)
{
	int ret;

	ret = aero_check_platform(wdev);
	if (ret)
		return ret;

	aero_device_bound(&aero.wmbc, wdev);
	return 0;
}

static void aero_wmbc_remove(struct wmi_device *wdev)
{
	aero_device_unbound(&aero.wmbc);
}

static int aero_event_probe(struct wmi_device *wdev, const void *context)
{
	int ret;

	ret = aero_check_platform(wdev);
	if (ret)
		return ret;

	mutex_lock(&aero.io_lock);
	aero.event = wdev;
	mutex_unlock(&aero.io_lock);

	return 0;
}

static void aero_event_remove(struct wmi_device *wdev)
{
	mutex_lock(&aero.io_lock);
	aero.event = NULL;
	mutex_unlock(&aero.io_lock);
}

/*
 * _Q45 -> SMGR(WEVN, WEVS) -> WMBC(0, 0x03, WEVN) -> Notify(AMW0, 0xD2) ->
 * cekirdek _WED(0xD2) cagirir -> DEVS tamponu doner (4 bayt; anlamli kismi
 * ilk iki bayt: [olay_no, durum]).
 *
 * Olay numaralarinin listesi DSDT'de YOK ve firmware'de izlenmedi. Bu yuzden
 * v1'de sparse_keymap BOS: gelen ciftleri logluyoruz, tablo bu logdan
 * olculerek doldurulacak (Fn+F1..F12 ve ozel tuslara basip dmesg okunur).
 * Olculmemis bir tus esleme tablosu koymak, olmayan bir yetenegi sunmak olur.
 */
static void aero_event_notify(struct wmi_device *wdev,
			      const struct wmi_buffer *data)
{
	const u8 *ev = data->data;

	dev_info(&wdev->dev, "EC olayi: no=0x%02x durum=0x%02x\n", ev[0], ev[1]);
}

static const struct wmi_device_id aero_wmbd_id_table[] = {
	{ AERO_WMI_GUID_WMBD, NULL },
	{ }
};

static const struct wmi_device_id aero_wmbc_id_table[] = {
	{ AERO_WMI_GUID_WMBC, NULL },
	{ }
};

static const struct wmi_device_id aero_event_id_table[] = {
	{ AERO_WMI_GUID_EVENT, NULL },
	{ }
};

static struct wmi_driver aero_wmbd_driver = {
	.driver = {
		.name = "aero_eg61h",
	},
	.id_table = aero_wmbd_id_table,
	.probe = aero_wmbd_probe,
	.remove = aero_wmbd_remove,
};

static struct wmi_driver aero_wmbc_driver = {
	.driver = {
		.name = "aero_eg61h_wmbc",
	},
	.id_table = aero_wmbc_id_table,
	.probe = aero_wmbc_probe,
	.remove = aero_wmbc_remove,
};

static struct wmi_driver aero_event_driver = {
	.driver = {
		.name = "aero_eg61h_evt",
	},
	.id_table = aero_event_id_table,
	/* _WED 4 bayt döndürüyor; ilk ikisi anlamlı, ikisini şart koşuyoruz */
	.min_event_size = 2,
	.probe = aero_event_probe,
	.remove = aero_event_remove,
	.notify_new = aero_event_notify,
};

MODULE_DEVICE_TABLE(wmi, aero_wmbd_id_table);
MODULE_DEVICE_TABLE(wmi, aero_wmbc_id_table);
MODULE_DEVICE_TABLE(wmi, aero_event_id_table);

/* ------------------------------------------------------------------------- *
 * Modül giriş/çıkış
 * ------------------------------------------------------------------------- */

/*
 * Sıra önemli: okuma yolu (WMBC) önce kaydedilir ki WMBD bağlandığında
 * aero_core_attach() kanıt okumasını yapabilsin. Sıra garanti değil — kod
 * her iki sıraya da dayanıklı (aero_device_bound ikisinin de hazır olmasını
 * bekliyor) — ama bu sıralama tipik durumda tek geçişte tamamlanmasını sağlar.
 */
static int __init aero_init(void)
{
	int ret;

	ret = wmi_driver_register(&aero_wmbc_driver);
	if (ret)
		return ret;

	ret = wmi_driver_register(&aero_event_driver);
	if (ret)
		goto err_wmbc;

	ret = wmi_driver_register(&aero_wmbd_driver);
	if (ret)
		goto err_event;

	return 0;

err_event:
	wmi_driver_unregister(&aero_event_driver);
err_wmbc:
	wmi_driver_unregister(&aero_wmbc_driver);
	return ret;
}

static void __exit aero_exit(void)
{
	wmi_driver_unregister(&aero_wmbd_driver);
	wmi_driver_unregister(&aero_event_driver);
	wmi_driver_unregister(&aero_wmbc_driver);
}

module_init(aero_init);
module_exit(aero_exit);

MODULE_AUTHOR("zixar");
MODULE_DESCRIPTION("Gigabyte AERO X16 1VH (EG61VH) platform driver");
MODULE_LICENSE("GPL");
