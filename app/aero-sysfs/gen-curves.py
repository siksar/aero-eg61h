#!/usr/bin/env python3
"""curves.rs üreteci — EC firmware imajından fan eğrisi tablolarını çıkarır.

KAYNAK: ~/Downloads/aero-ec/EG61H-EC-F00A.bin (BIOS FB0A / EC F00A)
Ofsetler ve yapı: ~/ecscope/docs/firmware-8051.md §4.

Tablo yapısı: 15 satır x 5 bayt (t1, t2, duty, bayrak1, bayrak2), adım 0x4B.
Satır 0 "fan kapalı" noktası (t1 = 0); EC XRAM'e kopyalanan 14 satır ondan
sonrakiler — ölçülen "14 satır x 3 bayt" ile birebir.

Grup içinde dört tablo: b0 = 0x00 (fan 0), 0x10 (fan 1), 0x30, 0x40.
0x30/0x40 HİÇ ÖLÇÜLMEDİ (bütün ölçümler pilde yapıldı) — çıkarılmıyorlar.
"""
import sys, pathlib

BIN = pathlib.Path.home() / "Downloads/aero-ec/EG61H-EC-F00A.bin"
ROW, ROWS, STRIDE = 5, 15, 0x4B

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
            out.append((ad, fan, off, imod, b0, t))
    for ad, fan, off, imod, b0, t in out:
        print(f"// {ad} fan{fan}  kaynak 0x{off:05X}  ic_mod 0x{imod:02X}  b0 0x{b0:02X}")
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
'''

def rust(out):
    L = [RS_BASLIK, f"\npub const EGRILER: [Egri; {len(out)}] = ["]
    for ad, fan, off, imod, b0, t in out:
        L.append(f"    // kaynak 0x{off:05X} · iç mod 0x{imod:02X} · b0 0x{b0:02X}")
        L.append(f"    Egri {{ mod_: FanMode::{ad}, fan: {fan}, kaynak: 0x{off:05X}, noktalar: [")
        for t1, t2, d in t:
            L.append(f"        Nokta {{ t1: {t1:3}, t2: {t2:3}, duty: {d:3} }},")
        L.append("    ] },")
    L.append("];")
    return "\n".join(L) + "\n"

if __name__ == "__main__":
    o = main()
    if "--rust" in sys.argv:
        p = pathlib.Path(__file__).parent / "src/curves.rs"
        p.write_text(rust(o))
        print(f"yazildi: {p} ({len(o)} egri)", file=sys.stderr)
