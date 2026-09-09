#!/usr/bin/env python3
"""curves.rs üreteci — EC firmware imajından fan eğrisi tablolarını çıkarır.

KAYNAK: ~/Downloads/aero-ec/EG61H-EC-F00A.bin (BIOS FB0A / EC F00A)
Ofsetler ve yapı: ~/ecscope/docs/firmware-8051.md §4.

Tablo yapısı: 15 satır x 5 bayt (t1, t2, duty, bayrak1, bayrak2), adım 0x4B.
Satır 0 "fan kapalı" noktası (t1 = 0); EC XRAM'e kopyalanan 14 satır ondan
sonrakiler — ölçülen "14 satır x 3 bayt" ile birebir.

Grup içinde dört tablo: b0 = 0x00 (fan 0), 0x10 (fan 1), 0x30, 0x40.
0x30/0x40 HİÇ ÖLÇÜLMEDİ (bütün ölçümler pilde yapıldı) — çıkarılmıyorlar.

Her eğri, kendisini seçen KURAL KAYDINI da taşıyor (§4.2, tablo 0x0616E).
Kayıt imajdan AYNEN okunuyor, yeniden kurulmuyor: arayüzün "bu eğri nereden
geliyor" sorusuna verdiği cevap böylece bir çıkarım değil, bir alıntı.
"""
import sys, pathlib

BIN = pathlib.Path.home() / "Downloads/aero-ec/EG61H-EC-F00A.bin"
ROW, ROWS, STRIDE = 5, 15, 0x4B

# Eğri tablolarını seçen kural tablosu — firmware-8051.md §4.2.
# Kayıt: { b0, 0x00, b2, mod, 0x00, 0xFF, adres_hi, adres_lo }, 24 kayıt + catch-all.
KURAL, KURAL_BOY, KURAL_SAYI = 0x0616E, 8, 24

# (taban, ic_mod, bizim_ad, belgelenen_tavan)  — firmware-8051.md §4.2 tablosu
GRUPLAR = [
    (0x05A66, 0x02, "Quiet",      29),
    (0x05B92, 0x00, "Responsive", 43),
    (0x05CBE, 0x04, "Balanced",   43),
    (0x05DEA, 0x01, "Gaming",     53),
    (0x05F16, 0x03, "Turbo",      63),
]

def tablo(buf, off):
    """14 veri satırı (satır 0 atlanıyor: t1=0, fan kapalı noktası)."""
    rows = [tuple(buf[off + i*ROW : off + i*ROW + 3]) for i in range(ROWS)]
    assert rows[0][0] == 0, f"0x{off:05X}: satir 0 t1={rows[0][0]}, 0 bekleniyordu"
    return rows[1:]

def kural(buf, off, imod, fan):
    """Verilen tablo ofsetine işaret eden kural kaydını imajdan bulur.

    Kayıt YENIDEN KURULMUYOR, bulunuyor: adres alanı eşleşen tek kayıt
    aranıyor ve `mod`/`b0` alanları beklentiyle karşılaştırılıyor. Eşleşme
    tek değilse ya da alanlar tutmuyorsa üreteç durur — belge ile imaj
    arasında sessiz bir kayma olmasın diye.
    """
    esler = []
    for i in range(KURAL_SAYI):
        r = buf[KURAL + i*KURAL_BOY : KURAL + (i+1)*KURAL_BOY]
        if (r[6] << 8 | r[7]) == off:
            esler.append((i, bytes(r)))
    assert len(esler) == 1, f"0x{off:05X}: {len(esler)} kural kaydi esledi, 1 bekleniyordu"
    i, r = esler[0]
    assert r[3] == imod, f"0x{off:05X}: kural mod=0x{r[3]:02X}, 0x{imod:02X} bekleniyordu"
    assert r[0] == fan * 0x10, f"0x{off:05X}: kural b0=0x{r[0]:02X}, fan{fan} bekleniyordu"
    return i, r

def main():
    buf = BIN.read_bytes()
    out = []
    for base, imod, ad, tavan in GRUPLAR:
        for fan, b0 in ((0, 0x00), (1, 0x10)):
            off = base + fan * STRIDE
            t = tablo(buf, off)
            # KENDI KENDINI DENETLEME: belgelenen tavan tutuyor mu?
            gercek = max(d for _, _, d in t)
            assert gercek == tavan, \
                f"{ad} fan{fan}: tavan {gercek}, belgede {tavan}"
            assert all(t[i][0] <= t[i+1][0] for i in range(len(t)-1)), \
                f"{ad} fan{fan}: t1 artan degil"
            ki, kr = kural(buf, off, imod, fan)
            assert kr[0] == b0
            out.append((ad, fan, off, imod, b0, ki, kr, t))
    for ad, fan, off, imod, b0, ki, kr, t in out:
        kh = " ".join(f"{b:02X}" for b in kr)
        print(f"// {ad} fan{fan}  kaynak 0x{off:05X}  ic_mod 0x{imod:02X}  b0 0x{b0:02X}"
              f"  kural[{ki}] 0x{KURAL + ki*KURAL_BOY:05X}: {kh}")
        for t1, t2, d in t:
            print(f"//   {t1:3d} {t2:3d} {d:3d}")
    return out


RS_BASLIK = '''// SPDX-License-Identifier: GPL-2.0-only
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

/// Eğri tablolarını seçen kural tablosunun imajdaki adresi
/// (`firmware-8051.md` §4.2). Kayıt boyu 8 bayt, 24 kayıt + bir catch-all.
pub const KURAL_TABLOSU: u32 = 0x0616E;

/// Bir fan için bir modun eğrisi.
#[derive(Debug, Clone, Copy)]
pub struct Egri {
    pub mod_: FanMode,
    /// 0 ya da 1. Hangi fanın neyi soğuttuğu ÖLÇÜLMEDİ.
    pub fan: u8,
    /// Firmware imajındaki kaynak ofseti — kanıt izlenebilir olsun diye.
    pub kaynak: u32,
    /// Bu tabloyu seçen kural kaydı, imajdan AYNEN (yeniden kurulmadı):
    /// `{ b0, 0x00, b2, iç_mod, 0x00, 0xFF, adres_hi, adres_lo }`.
    pub kural: [u8; 8],
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

    /// Kural kaydının `b0` alanı — grup içindeki tablo seçicisi.
    /// `0x00` = fan 0, `0x10` = fan 1. (`0x30`/`0x40` hiç görülmedi.)
    pub fn b0(&self) -> u8 {
        self.kural[0]
    }

    /// Kural kaydının `b2` alanı. Ana dizideki beş grupta `0x10`;
    /// grup 5'te (custom tohumu olduğu DÜŞÜNÜLEN kopya) `0x00`.
    pub fn b2(&self) -> u8 {
        self.kural[2]
    }

    /// EC'nin **iç** mod numarası. Sysfs adıyla aynı değil — "Dengeli"
    /// iç mod `0x04`, "Duyarlı" iç mod `0x00`. `aorus_laptop`'ın
    /// yanlış bildirdiği ayrım tam burada.
    pub fn ic_mod(&self) -> u8 {
        self.kural[3]
    }

    /// Kural kaydının işaret ettiği tablo adresi — [`Egri::kaynak`] ile
    /// aynı olmalı (bir test bunu sabitliyor).
    pub fn kural_adresi(&self) -> u32 {
        (self.kural[6] as u32) << 8 | self.kural[7] as u32
    }
}

/// Verilen mod ve fan için eğri.
pub fn egri(m: FanMode, fan: u8) -> Option<&'static Egri> {
    EGRILER.iter().find(|e| e.mod_ == m && e.fan == fan)
}
'''


# Testler de BURADA yaşıyor. Daha önce dosyanın sonuna elle eklenmişlerdi ve
# üreteç onları emitlemiyordu — bir sonraki `--rust` koşusu sessizce silecekti.
RS_TESTLER = '''
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

    /// Kural kaydı, taşıdığı eğriyi gerçekten gösteriyor mu. Kayıt imajdan
    /// AYNEN alındığı için bu, "üreteç doğru kaydı eşledi" denetimidir.
    #[test]
    fn kural_kaydi_kendi_tablosunu_gosteriyor() {
        for e in &EGRILER {
            assert_eq!(e.kural_adresi(), e.kaynak, "{:?} fan{}", e.mod_, e.fan);
            assert_eq!(e.b0(), e.fan * 0x10, "{:?} fan{}", e.mod_, e.fan);
            // Ana dizideki beş grubun hepsinde b2 = 0x10 (§4.2).
            assert_eq!(e.b2(), 0x10, "{:?} fan{}", e.mod_, e.fan);
            // Sabit alanlar: kayıt düzeni { b0, 00, b2, mod, 00, FF, hi, lo }.
            assert_eq!(e.kural[1], 0x00);
            assert_eq!(e.kural[4], 0x00);
            assert_eq!(e.kural[5], 0xFF);
        }
    }

    /// İç mod numaraları sysfs adlarıyla AYNI DEĞİL — `aorus_laptop`'ın
    /// yanlış bildirdiği ayrım burada sabitleniyor (`firmware-8051.md` §4.2).
    #[test]
    fn ic_mod_numaralari_belgeyle_ayni() {
        for (m, imod) in [
            (FanMode::Responsive, 0x00u8),
            (FanMode::Gaming, 0x01),
            (FanMode::Quiet, 0x02),
            (FanMode::Turbo, 0x03),
            (FanMode::Balanced, 0x04),
        ] {
            for fan in 0..2u8 {
                assert_eq!(egri(m, fan).unwrap().ic_mod(), imod, "{m:?} fan{fan}");
            }
        }
    }

    /// Aynı modun iki fanı aynı iç moda ait olmalı — grup içindeki iki tablo.
    #[test]
    fn iki_fan_ayni_gruptan() {
        for m in [
            FanMode::Quiet,
            FanMode::Balanced,
            FanMode::Responsive,
            FanMode::Gaming,
            FanMode::Turbo,
        ] {
            let a = egri(m, 0).unwrap();
            let b = egri(m, 1).unwrap();
            assert_eq!(a.ic_mod(), b.ic_mod(), "{m:?}");
            // Grup adımı 0x4B (15 satır × 5 bayt).
            assert_eq!(b.kaynak - a.kaynak, 0x4B, "{m:?}");
        }
    }
}
'''


def rust(out):
    L = [RS_BASLIK, f"\npub const EGRILER: [Egri; {len(out)}] = ["]
    for ad, fan, off, imod, b0, ki, kr, t in out:
        kh = ", ".join(f"0x{b:02X}" for b in kr)
        L.append(f"    // kaynak 0x{off:05X} · iç mod 0x{imod:02X} · b0 0x{b0:02X}"
                 f" · kural[{ki}] @ 0x{KURAL + ki*KURAL_BOY:05X}")
        L.append(f"    Egri {{ mod_: FanMode::{ad}, fan: {fan}, kaynak: 0x{off:05X},")
        L.append(f"           kural: [{kh}], noktalar: [")
        for t1, t2, d in t:
            L.append(f"        Nokta {{ t1: {t1:3}, t2: {t2:3}, duty: {d:3} }},")
        L.append("    ] },")
    L.append("];")
    return "\n".join(L) + "\n" + RS_TESTLER

if __name__ == "__main__":
    o = main()
    if "--rust" in sys.argv:
        p = pathlib.Path(__file__).parent / "src/curves.rs"
        p.write_text(rust(o))
        print(f"yazildi: {p} ({len(o)} egri)", file=sys.stderr)
