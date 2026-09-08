// SPDX-License-Identifier: GPL-2.0-only
// OTOMATİK ÜRETİLDİ — elle düzenleme. Üreteci: aero-sysfs/gen-curves.py
//
//! Fan eğrisi tabloları — EC firmware imajından çıkarılmış sabit veri.
//!
//! # Neden burada
//!
//! Eğriler EC'nin iç adres uzayında (`0xF8AC`/`0xF8E2`) ve oradan okumak
//! `EIDR` mailbox'ı gerektiriyor — yavaş (~600 ms/tablo) ve şu an sürücüden
//! erişilebilir DEĞİL (`ECTE` zaman aşımı denetimi için ACPI yolu yok, bkz.
//! `kernel/README.md`). Ama aynı tablolar firmware imajında da duruyor ve
//! **ölçülen sekiz aktif tablonun sekizi de kaynakta birebir eşleşti**
//! (`~/ecscope/docs/firmware-8051.md` §4.2).
//!
//! Yani arayüz eğrileri **hiçbir donanıma dokunmadan, anında** çizebiliyor.
//!
//! # DİKKAT: BASAMAK, EĞRİ DEĞİL
//!
//! `t1`/`t2`'nin ara değerlerde interpolasyon mu yoksa eşik mi olduğu
//! **ÖLÇÜLMEDİ** (`firmware-8051.md` §9, açık iş). Bu yüzden [`Egri::duty`]
//! basamak (step) semantiği kullanıyor: en son aşılan eşiğin duty'si.
//! Bu, en az iddia eden gösterim. Ölçüm interpolasyon çıkarırsa burası
//! değişir; o zamana kadar uydurma yapmıyoruz.
//!
//! # Sunulmayan iki tablo
//!
//! Her grupta dört tablo var (`b0` = 0x00/0x10/0x30/0x40). Yalnız `0x00`
//! (fan 0) ve `0x10` (fan 1) sunuluyor — **bütün ölçümler pilde yapıldığı
//! için `0x30`/`0x40` hiç görülmedi** ve ne oldukları açıklanmadı
//! (AC/DC ayrımı hipotezi 7 Eyl'de çürüdü).

use crate::FanMode;

/// Eğri üzerinde tek nokta. `t1`/`t2` kasıtlı olarak böyle adlandırıldı —
/// hangi sensöre baktıkları ölçülmedi, `cpu`/`gpu` demek yalan olurdu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Nokta {
    pub t1: u8,
    pub t2: u8,
    /// Fan görev çevrimi, yüzde.
    pub duty: u8,
}

/// Bir fan için bir modun eğrisi.
#[derive(Debug, Clone, Copy)]
pub struct Egri {
    pub mod_: FanMode,
    /// 0 ya da 1. Hangi fanın neyi soğuttuğu ÖLÇÜLMEDİ.
    pub fan: u8,
    /// Firmware imajındaki kaynak ofseti — kanıt izlenebilir olsun diye.
    pub kaynak: u32,
    pub noktalar: [Nokta; 14],
}

impl Egri {
    /// Verilen sıcaklıkta beklenen duty — **BASAMAK**, interpolasyon değil.
    ///
    /// İlk eşiğin altında 0 döner (fan durur; mod 4'te boşta gerçekten
    /// duruyor, ölçüldü).
    pub fn duty(&self, t1: u8) -> u8 {
        let mut d = 0;
        for n in &self.noktalar {
            if t1 >= n.t1 {
                d = n.duty;
            } else {
                break;
            }
        }
        d
    }

    /// Fanın dönmeye başladığı eşik.
    pub fn baslangic(&self) -> u8 {
        self.noktalar[0].t1
    }

    /// En yüksek duty.
    pub fn tavan(&self) -> u8 {
        self.noktalar.iter().map(|n| n.duty).max().unwrap_or(0)
    }
}

/// Verilen mod ve fan için eğri.
pub fn egri(m: FanMode, fan: u8) -> Option<&'static Egri> {
    EGRILER.iter().find(|e| e.mod_ == m && e.fan == fan)
}


pub const EGRILER: [Egri; 10] = [
    // kaynak 0x05A66 · iç mod 0x02 · b0 0x00
    Egri { mod_: FanMode::Quiet, fan: 0, kaynak: 0x05A66, noktalar: [
        Nokta { t1:  54, t2:  69, duty:  18 },
        Nokta { t1:  61, t2:  71, duty:  18 },
        Nokta { t1:  63, t2:  74, duty:  20 },
        Nokta { t1:  66, t2:  76, duty:  20 },
        Nokta { t1:  68, t2:  79, duty:  21 },
        Nokta { t1:  71, t2:  81, duty:  21 },
        Nokta { t1:  73, t2:  84, duty:  23 },
        Nokta { t1:  76, t2:  87, duty:  23 },
        Nokta { t1:  79, t2:  90, duty:  23 },
        Nokta { t1:  82, t2:  92, duty:  23 },
        Nokta { t1:  84, t2:  94, duty:  23 },
        Nokta { t1:  86, t2:  96, duty:  23 },
        Nokta { t1:  88, t2:  98, duty:  26 },
        Nokta { t1:  90, t2: 100, duty:  29 },
    ] },
    // kaynak 0x05AB1 · iç mod 0x02 · b0 0x10
    Egri { mod_: FanMode::Quiet, fan: 1, kaynak: 0x05AB1, noktalar: [
        Nokta { t1:  50, t2:  60, duty:  18 },
        Nokta { t1:  54, t2:  64, duty:  21 },
        Nokta { t1:  58, t2:  68, duty:  21 },
        Nokta { t1:  62, t2:  71, duty:  23 },
        Nokta { t1:  65, t2:  72, duty:  26 },
        Nokta { t1:  68, t2:  75, duty:  29 },
        Nokta { t1:  70, t2:  78, duty:  29 },
        Nokta { t1:  73, t2:  81, duty:  29 },
        Nokta { t1:  76, t2:  84, duty:  29 },
        Nokta { t1:  79, t2:  87, duty:  29 },
        Nokta { t1:  82, t2:  90, duty:  29 },
        Nokta { t1:  85, t2:  93, duty:  29 },
        Nokta { t1:  88, t2:  96, duty:  29 },
        Nokta { t1:  91, t2: 100, duty:  29 },
    ] },
    // kaynak 0x05B92 · iç mod 0x00 · b0 0x00
    Egri { mod_: FanMode::Responsive, fan: 0, kaynak: 0x05B92, noktalar: [
        Nokta { t1:  40, t2:  54, duty:  18 },
        Nokta { t1:  46, t2:  59, duty:  20 },
        Nokta { t1:  51, t2:  64, duty:  21 },
        Nokta { t1:  56, t2:  68, duty:  23 },
        Nokta { t1:  60, t2:  72, duty:  26 },
        Nokta { t1:  64, t2:  75, duty:  29 },
        Nokta { t1:  67, t2:  81, duty:  33 },
        Nokta { t1:  76, t2:  84, duty:  33 },
        Nokta { t1:  79, t2:  87, duty:  33 },
        Nokta { t1:  82, t2:  90, duty:  33 },
        Nokta { t1:  85, t2:  93, duty:  33 },
        Nokta { t1:  88, t2:  96, duty:  33 },
        Nokta { t1:  91, t2:  98, duty:  38 },
        Nokta { t1:  93, t2: 100, duty:  43 },
    ] },
    // kaynak 0x05BDD · iç mod 0x00 · b0 0x10
    Egri { mod_: FanMode::Responsive, fan: 1, kaynak: 0x05BDD, noktalar: [
        Nokta { t1:  48, t2:  57, duty:  18 },
        Nokta { t1:  51, t2:  60, duty:  20 },
        Nokta { t1:  54, t2:  62, duty:  21 },
        Nokta { t1:  56, t2:  64, duty:  23 },
        Nokta { t1:  58, t2:  66, duty:  26 },
        Nokta { t1:  60, t2:  68, duty:  29 },
        Nokta { t1:  62, t2:  70, duty:  33 },
        Nokta { t1:  64, t2:  74, duty:  38 },
        Nokta { t1:  68, t2:  82, duty:  43 },
        Nokta { t1:  77, t2:  86, duty:  43 },
        Nokta { t1:  81, t2:  90, duty:  43 },
        Nokta { t1:  85, t2:  94, duty:  43 },
        Nokta { t1:  89, t2:  98, duty:  43 },
        Nokta { t1:  93, t2: 100, duty:  43 },
    ] },
    // kaynak 0x05CBE · iç mod 0x04 · b0 0x00
    Egri { mod_: FanMode::Balanced, fan: 0, kaynak: 0x05CBE, noktalar: [
        Nokta { t1:  54, t2:  70, duty:  18 },
        Nokta { t1:  60, t2:  74, duty:  20 },
        Nokta { t1:  66, t2:  77, duty:  21 },
        Nokta { t1:  69, t2:  80, duty:  23 },
        Nokta { t1:  72, t2:  83, duty:  26 },
        Nokta { t1:  75, t2:  86, duty:  29 },
        Nokta { t1:  78, t2:  88, duty:  33 },
        Nokta { t1:  80, t2:  90, duty:  33 },
        Nokta { t1:  82, t2:  92, duty:  33 },
        Nokta { t1:  84, t2:  94, duty:  33 },
        Nokta { t1:  86, t2:  95, duty:  33 },
        Nokta { t1:  87, t2:  96, duty:  33 },
        Nokta { t1:  88, t2:  98, duty:  38 },
        Nokta { t1:  90, t2: 100, duty:  43 },
    ] },
    // kaynak 0x05D09 · iç mod 0x04 · b0 0x10
    Egri { mod_: FanMode::Balanced, fan: 1, kaynak: 0x05D09, noktalar: [
        Nokta { t1:  48, t2:  57, duty:  18 },
        Nokta { t1:  51, t2:  60, duty:  20 },
        Nokta { t1:  54, t2:  62, duty:  21 },
        Nokta { t1:  56, t2:  64, duty:  23 },
        Nokta { t1:  58, t2:  66, duty:  26 },
        Nokta { t1:  60, t2:  68, duty:  29 },
        Nokta { t1:  62, t2:  70, duty:  33 },
        Nokta { t1:  64, t2:  74, duty:  38 },
        Nokta { t1:  68, t2:  82, duty:  43 },
        Nokta { t1:  77, t2:  86, duty:  43 },
        Nokta { t1:  81, t2:  90, duty:  43 },
        Nokta { t1:  85, t2:  94, duty:  43 },
        Nokta { t1:  89, t2:  98, duty:  43 },
        Nokta { t1:  93, t2: 100, duty:  43 },
    ] },
    // kaynak 0x05DEA · iç mod 0x01 · b0 0x00
    Egri { mod_: FanMode::Gaming, fan: 0, kaynak: 0x05DEA, noktalar: [
        Nokta { t1:  40, t2:  54, duty:  18 },
        Nokta { t1:  46, t2:  59, duty:  20 },
        Nokta { t1:  51, t2:  64, duty:  21 },
        Nokta { t1:  56, t2:  68, duty:  23 },
        Nokta { t1:  60, t2:  72, duty:  26 },
        Nokta { t1:  64, t2:  75, duty:  29 },
        Nokta { t1:  67, t2:  80, duty:  33 },
        Nokta { t1:  72, t2:  84, duty:  33 },
        Nokta { t1:  76, t2:  88, duty:  33 },
        Nokta { t1:  80, t2:  92, duty:  33 },
        Nokta { t1:  84, t2:  96, duty:  38 },
        Nokta { t1:  86, t2:  97, duty:  43 },
        Nokta { t1:  88, t2: 105, duty:  48 },
        Nokta { t1:  90, t2: 110, duty:  53 },
    ] },
    // kaynak 0x05E35 · iç mod 0x01 · b0 0x10
    Egri { mod_: FanMode::Gaming, fan: 1, kaynak: 0x05E35, noktalar: [
        Nokta { t1:  46, t2:  54, duty:  18 },
        Nokta { t1:  49, t2:  57, duty:  20 },
        Nokta { t1:  52, t2:  60, duty:  21 },
        Nokta { t1:  55, t2:  63, duty:  23 },
        Nokta { t1:  58, t2:  66, duty:  26 },
        Nokta { t1:  61, t2:  68, duty:  29 },
        Nokta { t1:  63, t2:  70, duty:  33 },
        Nokta { t1:  65, t2:  72, duty:  38 },
        Nokta { t1:  67, t2:  74, duty:  43 },
        Nokta { t1:  69, t2:  76, duty:  48 },
        Nokta { t1:  71, t2:  80, duty:  53 },
        Nokta { t1:  75, t2:  84, duty:  53 },
        Nokta { t1:  79, t2:  88, duty:  53 },
        Nokta { t1:  83, t2: 100, duty:  53 },
    ] },
    // kaynak 0x05F16 · iç mod 0x03 · b0 0x00
    Egri { mod_: FanMode::Turbo, fan: 0, kaynak: 0x05F16, noktalar: [
        Nokta { t1:  36, t2:  48, duty:  63 },
        Nokta { t1:  43, t2:  54, duty:  63 },
        Nokta { t1:  49, t2:  60, duty:  63 },
        Nokta { t1:  55, t2:  66, duty:  63 },
        Nokta { t1:  61, t2:  72, duty:  63 },
        Nokta { t1:  67, t2:  76, duty:  63 },
        Nokta { t1:  71, t2:  80, duty:  63 },
        Nokta { t1:  75, t2:  84, duty:  63 },
        Nokta { t1:  79, t2:  88, duty:  63 },
        Nokta { t1:  83, t2:  92, duty:  63 },
        Nokta { t1:  87, t2:  94, duty:  63 },
        Nokta { t1:  89, t2:  96, duty:  63 },
        Nokta { t1:  91, t2: 105, duty:  63 },
        Nokta { t1: 100, t2: 110, duty:  63 },
    ] },
    // kaynak 0x05F61 · iç mod 0x03 · b0 0x10
    Egri { mod_: FanMode::Turbo, fan: 1, kaynak: 0x05F61, noktalar: [
        Nokta { t1:  46, t2:  54, duty:  63 },
        Nokta { t1:  49, t2:  57, duty:  63 },
        Nokta { t1:  52, t2:  60, duty:  63 },
        Nokta { t1:  55, t2:  63, duty:  63 },
        Nokta { t1:  58, t2:  66, duty:  63 },
        Nokta { t1:  61, t2:  68, duty:  63 },
        Nokta { t1:  63, t2:  70, duty:  63 },
        Nokta { t1:  65, t2:  72, duty:  63 },
        Nokta { t1:  67, t2:  74, duty:  63 },
        Nokta { t1:  71, t2:  76, duty:  63 },
        Nokta { t1:  75, t2:  80, duty:  63 },
        Nokta { t1:  79, t2:  84, duty:  63 },
        Nokta { t1:  83, t2:  88, duty:  63 },
        Nokta { t1:  87, t2: 100, duty:  63 },
    ] },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bes_modun_ikisi_de_fani_var() {
        for m in [
            FanMode::Quiet,
            FanMode::Balanced,
            FanMode::Responsive,
            FanMode::Gaming,
            FanMode::Turbo,
        ] {
            for fan in 0..2u8 {
                assert!(egri(m, fan).is_some(), "{m:?} fan{fan} yok");
            }
        }
        assert_eq!(EGRILER.len(), 10);
    }

    #[test]
    fn sicakliklar_artan_duty_makul() {
        for e in &EGRILER {
            for w in e.noktalar.windows(2) {
                assert!(w[0].t1 <= w[1].t1, "{:?} fan{} t1 azaliyor", e.mod_, e.fan);
            }
            for n in &e.noktalar {
                assert!(n.duty <= 100, "duty {} > 100", n.duty);
            }
        }
    }

    /// Belgelenen tavanlar (`firmware-8051.md` §4.2) — üreteç de denetliyor,
    /// burada ikinci kez sabitleniyor ki veri elle bozulursa test düşsün.
    #[test]
    fn tavanlar_belgeyle_ayni() {
        for (m, tavan) in [
            (FanMode::Quiet, 29u8),
            (FanMode::Responsive, 43),
            (FanMode::Balanced, 43),
            (FanMode::Gaming, 53),
            (FanMode::Turbo, 63),
        ] {
            assert_eq!(egri(m, 0).unwrap().tavan(), tavan, "{m:?}");
        }
    }

    /// `kernel/README.md`'nin mod özetleriyle tutarlı mı.
    #[test]
    fn baslangic_esikleri_ozetle_ayni() {
        assert_eq!(egri(FanMode::Quiet, 0).unwrap().baslangic(), 54);
        assert_eq!(egri(FanMode::Balanced, 0).unwrap().baslangic(), 54);
        assert_eq!(egri(FanMode::Responsive, 0).unwrap().baslangic(), 40);
        assert_eq!(egri(FanMode::Gaming, 0).unwrap().baslangic(), 40);
        assert_eq!(egri(FanMode::Turbo, 0).unwrap().baslangic(), 36);
    }

    #[test]
    fn duty_basamak_semantigi() {
        let e = egri(FanMode::Quiet, 0).unwrap();
        // İlk eşiğin ALTINDA fan durur.
        assert_eq!(e.duty(0), 0);
        assert_eq!(e.duty(e.baslangic() - 1), 0);
        // Eşikte ilk duty.
        assert_eq!(e.duty(e.baslangic()), e.noktalar[0].duty);
        // İki eşik ARASINDA: BASAMAK — alttaki değerde kalır, interpolasyon YOK.
        let a = e.noktalar[0];
        let b = e.noktalar[1];
        if b.t1 > a.t1 + 1 {
            assert_eq!(e.duty(a.t1 + 1), a.duty, "interpolasyon yapiliyor!");
        }
        // Son eşiğin üstünde tavan.
        assert_eq!(e.duty(255), e.tavan());
    }

    /// Turbo'nun eğrisi düz — 7 Eyl'de canlı doğrulandı (boşta 37 °C'de
    /// fanlar 0'dan ~7000 rpm'e çıktı).
    #[test]
    fn turbo_duz() {
        let e = egri(FanMode::Turbo, 0).unwrap();
        assert!(e.noktalar.iter().all(|n| n.duty == 63), "turbo duz degil");
    }

    #[test]
    fn kaynak_ofsetleri_belgeyle_ayni() {
        assert_eq!(egri(FanMode::Quiet, 0).unwrap().kaynak, 0x05A66);
        assert_eq!(egri(FanMode::Balanced, 0).unwrap().kaynak, 0x05CBE);
        assert_eq!(egri(FanMode::Turbo, 0).unwrap().kaynak, 0x05F16);
    }
}
