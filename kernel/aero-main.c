// SPDX-License-Identifier: GPL-2.0-only
/*
 * aero_eg61h — Gigabyte AERO X16 1VH (SKU EG61VH) platform sürücüsü
 *
 * ADIM 1: iskelet. WMI bus'a gerçek bağlanma + WMBC/WMBD sarmalayıcıları.
 * Bu adımda hiçbir sysfs düğümü, hwmon kanalı ya da yazma yolu YOK.
 * Doğrulama ölçütü: /sys/bus/wmi/drivers/aero_eg61h görünür, dmesg temiz.
 *
 * This driver is intentionally hardware-specific. Only measured capabilities
 * are exposed: the firmware accepts writes to several registers that do not
 * affect fan speed, so those registers are not presented as controls.
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
#include <linux/pm.h>
#include <linux/slab.h>
#include <linux/wmi.h>

#include "aero-eg61h.h"

static bool force;
module_param(force, bool, 0444);
MODULE_PARM_DESC(force,
	"bind even when DMI does not match or a conflicting vendor driver is loaded (default: no)");

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
		pr_debug("WMBC 0x%02x is not an integer (type %u) — unknown selector?\n",
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
 * Bağlanma kanıtı + üst katmanların kurulması.
 *
 * Kanıt okuması tek seferlik — probe anında, bir kez. YOKLAMA DEĞİL: bu sürücü
 * boştayken hiçbir şey okumaz (4.28 W boşta güç bütçesi kuralı). hwmon da
 * kendiliğinden okumaz; her okuma userspace bir dosyayı okuduğunda olur.
 */
static void aero_core_attach(void)
{
	u32 cpu = 0, rpm1 = 0, rpm2 = 0;
	struct device *parent;
	u8 pattern = 0xff;
	int ret;

	aero_ec_lock();
	/* İşaretçiyi kilit altında yakala: eşzamanlı bir remove onu
	 * sıfırlayabilir ve hwmon ebeveyni olarak kullanacağız. */
	parent = aero.wmbd ? &aero.wmbd->dev : NULL;
	ret = __aero_ec_read(AERO_RD_CPU_TEMP, 0, &cpu);
	if (!ret)
		ret = __aero_ec_read(AERO_RD_FAN1_RPM, 0, &rpm1);
	if (!ret)
		ret = __aero_ec_read(AERO_RD_FAN2_RPM, 0, &rpm2);
	if (!ret)
		ret = __aero_fan_read_pattern(&pattern);
	aero_ec_unlock();

	if (ret) {
		pr_warn("EC read path failed (%d) — unexpected WMBC response\n",
			ret);
		return;
	}

	pr_info("EC read path active: CPU %u C, fans %u/%u rpm\n",
		cpu, rpm1, rpm2);
	pr_info("fan mode: 0x%02x (%s)\n", pattern, aero_fan_mode_name(pattern));

	if (!parent) {
		pr_warn("WMBD device disappeared — hwmon will not be registered\n");
		return;
	}

	ret = aero_hwmon_init(parent);
	if (ret)
		pr_warn("could not register hwmon (%d) — sensor channels unavailable\n", ret);
	else
		pr_info("hwmon ready: temp1 (CPU), fan1, fan2 — read-only\n");

	ret = aero_battery_init(parent);
	if (ret)
		pr_warn("charge limit unavailable (%d)\n", ret);

	ret = aero_profile_init(parent);
	if (ret)
		pr_warn("could not register platform_profile (%d)\n", ret);
	else
		pr_info("platform_profile ready: 0xED only (fan mode is separate)\n");
}


static void aero_core_detach(void)
{
	aero_profile_exit();
	aero_battery_exit();
	aero_hwmon_exit();
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
 * Another platform driver may create its own device and call the same WMI
 * methods directly. If both drivers are loaded, fan-mode writes can race, so
 * binding is refused instead of allowing two writers to run silently.
 */
static bool aero_conflicting_driver_present(void)
{
	struct device *dev;

	/* The legacy platform device keeps this historical kernel name. */
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
				 "this machine is not EG61VH — refusing to bind (use force=1 to override)\n");
			return -ENODEV;
		}
		dev_warn(&wdev->dev,
			 "force=1: DMI did not match; selector meanings are UNVERIFIED\n");
	}

	if (aero_conflicting_driver_present()) {
		if (!force) {
			dev_err(&wdev->dev,
				"another vendor driver is loaded — concurrent WMI calls could race.\n");
			dev_err(&wdev->dev,
				"remove the conflicting driver first, then load aero_eg61h.\n");
			return -EBUSY;
		}
		dev_warn(&wdev->dev,
			 "force=1: a conflicting driver is loaded; fan-mode writes may race\n");
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

	dev_info(&wdev->dev, "EC event: code=0x%02x state=0x%02x\n", ev[0], ev[1]);
}

/*
 * Uyanış kancası. EC şarj limitini uyanışta geri alıyor (6 Eyl ölçümü), yani
 * ayarın kalıcı olması TAM OLARAK buradan geliyor.
 *
 * NOT: bu geri çağrının gerçekten çalıştığı, uyanış logundaki "uyanis: ..."
 * satırıyla DOĞRULANMALI. PM çekirdeği sürücünün pm ops'unu yalnız bus kendi
 * bir geri çağrı sunmadığında çağırıyor; WMI bus'ın pm ops'u bu çekirdekte
 * incelenemedi (dev çıktısında drivers/ yok). Satır görünmezse yol
 * register_pm_notifier(PM_POST_SUSPEND) olarak değişecek — orası bus'tan
 * bağımsız.
 */
static int aero_resume(struct device *dev)
{
	aero_battery_resume();
	return 0;
}

static DEFINE_SIMPLE_DEV_PM_OPS(aero_pm_ops, NULL, aero_resume);

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
		.pm   = pm_sleep_ptr(&aero_pm_ops),
		/* fan_mode + fan_mode_choices bu cihazın altında görünür */
		.dev_groups = aero_wmbd_groups,
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
