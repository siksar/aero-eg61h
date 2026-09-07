// SPDX-License-Identifier: GPL-2.0-only
/*
 * aero_eg61h — pil katmanı (ADIM 3): şarj limiti
 *
 * Sürücünün İLK YAZMA YOLU. Standart ABI:
 *   /sys/class/power_supply/BAT1/charge_control_end_threshold   (yüzde)
 *
 * Özel bir `charge_limit` düğümü SUNULMUYOR — aorus_laptop'ınki bu; standart
 * karşılığı varken özel düğüm açmak, aracın onu tanımaması demek.
 * `charge_mode` (WMBD 0x64, BCPS) da yok: anlamı DSDT'den doğrulanamıyor ve
 * standart bir karşılığı yok.
 *
 * ------------------------------------------------------------------------
 * İKİ ÖLÇÜLMÜŞ GERÇEK, İKİSİ DE BU DOSYANIN TASARIMINI BELİRLİYOR
 * ------------------------------------------------------------------------
 *
 * 1. WMBD'nin DÖNÜŞ DEĞERİ HİÇBİR BİLGİ TAŞIMIYOR (6 Eyl 2026, 0xC7 ile
 *    ölçüldü): tanınmayan seçici bile girdiyi yankılıyor, yani "yazım tuttu"
 *    ile "seçici yok" ayırt edilemiyor. Bu yüzden her yazımdan sonra
 *    WMBC 0x65 ile GERİ OKUYUP karşılaştırıyoruz. Yazma+okuma tek kilit
 *    altında yapılıyor ki araya başka bir yazan girmesin.
 *
 * 2. EC LİMİTİ UYANIŞTA GERİ ALIYOR (6 Eyl 2026 ölçümü: boot servisi 60
 *    yazmıştı, sysfs değişim zamanı hâlâ boot anındayken değer %100'e
 *    dönmüştü — yani kimse 100 yazmadı, EC kendi geri aldı). Bu yüzden
 *    uyanışta yeniden uygulayan bir kanca var. Bu adımın asıl işi bu;
 *    yazmanın kendisi kolay kısım.
 */

#define pr_fmt(fmt) KBUILD_MODNAME ": " fmt

#include <linux/device.h>
#include <linux/errno.h>
#include <linux/kernel.h>
#include <linux/power_supply.h>

#include "aero-eg61h.h"

/* Ölçüldü: bu makinede pil `BAT1` (tek pil, ACAD ayrı). */
#define AERO_BATTERY_NAME	"BAT1"

static struct power_supply *aero_battery;

/*
 * Uyanışta yeniden uygulanacak değer. -1 = "biz hiç yazmadık", o durumda
 * uyanışta hiçbir şey yapmıyoruz — kullanıcının dokunmadığı bir ayarı
 * sürücünün zorlaması yanlış olur.
 */
static int aero_charge_limit = -1;

static int aero_charge_limit_read(u32 *pct)
{
	return aero_ec_read(AERO_RD_CHARGE_LIMIT, 0, pct);
}

/*
 * Yaz + geri oku + karşılaştır. Üçü tek kilit altında: WMBD'nin dönüşü
 * bilgi taşımadığı için doğrulama SADECE geri okumayla mümkün, ve araya
 * başka bir yazan girerse yanlış hüküm veririz.
 */
static int aero_charge_limit_write(u32 pct)
{
	u32 back = 0;
	int ret;

	aero_ec_lock();
	ret = __aero_ec_write(AERO_WR_CHARGE_LIMIT, pct);
	if (!ret)
		ret = __aero_ec_read(AERO_RD_CHARGE_LIMIT, 0, &back);
	aero_ec_unlock();

	if (ret)
		return ret;

	if (back != pct) {
		pr_warn("sarj limiti %u%% yazildi ama EC %u%% okuyor — yazim tutmadi\n",
			pct, back);
		return -EIO;
	}

	return 0;
}

static int aero_battery_get(struct power_supply *psy,
			    const struct power_supply_ext *ext, void *data,
			    enum power_supply_property psp,
			    union power_supply_propval *val)
{
	u32 pct;
	int ret;

	if (psp != POWER_SUPPLY_PROP_CHARGE_CONTROL_END_THRESHOLD)
		return -EINVAL;

	ret = aero_charge_limit_read(&pct);
	if (ret)
		return ret;

	val->intval = pct;
	return 0;
}

static int aero_battery_set(struct power_supply *psy,
			    const struct power_supply_ext *ext, void *data,
			    enum power_supply_property psp,
			    const union power_supply_propval *val)
{
	int ret;

	if (psp != POWER_SUPPLY_PROP_CHARGE_CONTROL_END_THRESHOLD)
		return -EINVAL;

	/*
	 * ASL bir aralık dayatmıyor (BCPC 8 bit, sınırsız). Standart ABI yüzde
	 * diyor, o yüzden 1-100. 0 REDDEDİLİYOR: anlamı ölçülmedi ve "hiç şarj
	 * etme" olabilir — ölçülmemiş bir davranışı kullanıcıya açmıyoruz.
	 * Canlı ölçülmüş değerler: 60 (tuttu, 30 s) ve 100.
	 */
	if (val->intval < 1 || val->intval > 100)
		return -EINVAL;

	ret = aero_charge_limit_write(val->intval);
	if (ret)
		return ret;

	/* Uyanış kancasının yeniden uygulayacağı değer artık bu. */
	aero_charge_limit = val->intval;
	return 0;
}

static int aero_battery_is_writeable(struct power_supply *psy,
				     const struct power_supply_ext *ext,
				     void *data, enum power_supply_property psp)
{
	return psp == POWER_SUPPLY_PROP_CHARGE_CONTROL_END_THRESHOLD;
}

static const enum power_supply_property aero_battery_props[] = {
	POWER_SUPPLY_PROP_CHARGE_CONTROL_END_THRESHOLD,
};

static const struct power_supply_ext aero_battery_ext = {
	.name			= "aero_eg61h",
	.properties		= aero_battery_props,
	.num_properties		= ARRAY_SIZE(aero_battery_props),
	.get_property		= aero_battery_get,
	.set_property		= aero_battery_set,
	.property_is_writeable	= aero_battery_is_writeable,
};

/*
 * Uyanışta yeniden uygula. EC limiti kendi geri alıyor (ölçüldü), yani bu
 * kanca olmadan ayar sessizce kayboluyor — kullanıcının fark etmesi günler
 * sürebilecek türden bir sessiz gerileme.
 */
void aero_battery_resume(void)
{
	u32 now = 0;
	int ret;

	if (!aero_battery || aero_charge_limit < 0)
		return;

	ret = aero_charge_limit_read(&now);
	if (!ret && now == (u32)aero_charge_limit) {
		pr_info("uyanis: sarj limiti %d%% korunmus, dokunulmadi\n",
			aero_charge_limit);
		return;
	}

	pr_info("uyanis: sarj limiti %u%% okundu, %d%% yeniden uygulaniyor\n",
		now, aero_charge_limit);

	ret = aero_charge_limit_write(aero_charge_limit);
	if (ret)
		pr_warn("uyanis: sarj limiti yeniden uygulanamadi (%d)\n", ret);
	else
		power_supply_changed(aero_battery);
}

int aero_battery_init(struct device *parent)
{
	u32 pct = 0;
	int ret;

	aero_battery = power_supply_get_by_name(AERO_BATTERY_NAME);
	if (!aero_battery) {
		pr_warn("%s bulunamadi — sarj limiti sunulmuyor\n",
			AERO_BATTERY_NAME);
		return -ENODEV;
	}

	ret = power_supply_register_extension(aero_battery, &aero_battery_ext,
					      parent, NULL);
	if (ret) {
		power_supply_put(aero_battery);
		aero_battery = NULL;
		return ret;
	}

	if (!aero_charge_limit_read(&pct))
		pr_info("sarj limiti hazir: %s/charge_control_end_threshold = %u%%\n",
			AERO_BATTERY_NAME, pct);

	return 0;
}

void aero_battery_exit(void)
{
	if (!aero_battery)
		return;

	power_supply_unregister_extension(aero_battery, &aero_battery_ext);
	power_supply_put(aero_battery);
	aero_battery = NULL;
	aero_charge_limit = -1;
}
