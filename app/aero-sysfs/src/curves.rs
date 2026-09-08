// SPDX-License-Identifier: GPL-2.0-only
//! EC firmware'inden çıkarılmış fan eğrisi tabloları — derleme zamanı sabit veri.
//!
//! # Bu dosya ne DEĞİL
//!
//! Bu bir denetim (control) katmanı değil. Sürücü eğri **yazamıyor**
//! (`XFNW` / `0x68` ölü protokol, `FDTY`/`FAN1` ölü yazmaç — ölçüldü). Burada
//! olan tek şey, EC'nin **kendi** kullandığı tabloların okunabilir bir kopyası:
//! GUI "sessiz modda fan 68 °C'de %21'e çıkar" diyebilsin diye.
//!
//! # Kaynak ve izlenebilirlik
//!
//! Tablolar `EG61H-EC-F00A.bin` (BIOS FB0A / EC F00A) imajından, `0x0616E`'deki
//! 8 baytlık kural tablosunun işaret ettiği adreslerden alındı. Kural tablosu
//! kaydı: `{ b0, 0x00, b2, mod, 0x00, 0xFF, adres_hi, adres_lo }`.
//! Çözümleme `~/ecscope/docs/firmware-8051.md` §4'te.
//!
//! Her tablo dosyada **15 satır × 5 bayt** (`t1, t2, duty, bayrak1, bayrak2`,
//! adım `0x4B`). Satır 0 `t1 = 0` başlığıdır ve EC XRAM'e **kopyalanmaz**;
//! canlı ölçümde görülen 14 satır × 3 bayt onun ardındaki veri satırlarıdır
//! (`0xF8AC` / `0xF8E2`). Bu yüzden [`Curve::points`] 14 elemanlı, başlık ayrı
//! alanda ([`Curve::header`]) ve yorumsuz duruyor.
//!
//! Grup içinde `b0 = 0x00` → fan 0, `b0 = 0x10` → fan 1 (7 Eyl 2026'da canlı
//! doğrulandı: AC'de mod `0x01` yüklenince kayıt 0 = `0x05A66`,
//! kayıt 1 = `0x05AB1`). Aynı gruptaki `b0 = 0x30` ve `0x40` tablolarının ne
//! olduğu **bilinmiyor** — AC/DC hipotezi ölçümle çürüdü — bu yüzden buraya
//! **alınmadılar**.
//!
//! # ÖLÇÜLMEMİŞ OLAN — ve API'nin neden BASAMAK sunduğu
//!
//! `t1` ve `t2`'nin ara sıcaklıklarda nasıl değerlendirildiği **ölçülmedi**.
//! Değerlendirici kod firmware'de bulunamadı; `t2`'nin ikinci bir eşik mi,
//! histerezis tavanı mı, başka bir sensör mü olduğu bilinmiyor (`t2` bazı
//! satırlarda 100'ü aşıyor: gaming/turbo fan 0'da 105 ve 110).
//!
//! Bu yüzden [`Curve::duty_step`] **interpolasyon YAPMAZ**. `t1`'i eşik kabul
//! eden düz bir basamak fonksiyonudur: sıcaklığın geçtiği **son** satırın
//! `duty`'sini döner. Bu, tablonun kendi verisinin en zayıf yorumu — gerçek EC
//! davranışı bundan daha yumuşak (rampalı) olabilir. Eğri "gerçekte" ne yapıyor
//! sorusunun cevabı ancak sıcaklık/rpm ölçümüyle gelir; o ölçüm yapılana kadar
//! GUI'de gösterilen şey "tablo bunu diyor", "fan bunu yapıyor" değil.
//!
//! `t1`'in CPU sıcaklığı eşiği olduğu tek başına ölçülmüş değil, ama beş modun
//! [`FanMode::describe`] özetiyle ("54 °C'de başlar" vb.) birebir tutuyor —
//! `curve_ozetler_fan_mode_describe_ile_tutar` testi bunu bağlıyor.

use crate::FanMode;

/// Kural tablosunun firmware ofseti — kanıt zincirinin başı.
pub const RULE_TABLE_OFFSET: u32 = 0x0616E;
/// Bir eğri tablosunun boyu / gruptaki adım (15 satır × 5 bayt).
pub const TABLE_STRIDE: u32 = 0x4B;
/// Bir tablodaki veri satırı sayısı (başlık satırı hariç).
pub const POINTS: usize = 14;

/// İki fan. Kural tablosundaki `b0` alanının ölçülmüş anlamı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fan {
    /// `b0 = 0x00` — hwmon `fan1_input`, XRAM kaydı `0xF8AC`.
    Fan0,
    /// `b0 = 0x10` — hwmon `fan2_input`, XRAM kaydı `0xF8E2`.
    Fan1,
}

/// Eğrinin bir satırı. Ham firmware baytları, yorumsuz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    /// Alt eşik (°C). [`Curve::duty_step`] yalnız bunu kullanır.
    pub t1_c: u8,
    /// İkinci bayt. **Anlamı ölçülmedi** — histerezis, üst sınır ya da başka
    /// bir sensör olabilir. Hiçbir hesapta kullanılmıyor, kanıt olsun diye var.
    pub t2_c: u8,
    /// Fan doluluğu, yüzde (`0..=100`; gözlenen tavan 63).
    pub duty_pct: u8,
}

const fn p(t1_c: u8, t2_c: u8, duty_pct: u8) -> Point {
    Point { t1_c, t2_c, duty_pct }
}

/// Tek bir mod + fan eğrisi, kaynak ofsetiyle birlikte.
#[derive(Debug, Clone, Copy)]
pub struct Curve {
    pub mode: FanMode,
    pub fan: Fan,
    /// Tablonun firmware imajındaki ofseti — kanıt buraya kadar izlenir.
    pub src_offset: u32,
    /// Kural tablosundaki `mod` alanı (firmware'in iç mod numarası).
    pub rule_mod: u8,
    /// Kural tablosundaki `b0` alanı (`0x00` = fan 0, `0x10` = fan 1).
    pub rule_b0: u8,
    /// `t1 = 0` başlık satırı. EC XRAM'e kopyalanmaz; **yorumu ölçülmedi**,
    /// bu yüzden [`Curve::duty_step`] onu kullanmaz.
    pub header: Point,
    /// XRAM'e kopyalanan 14 veri satırı, `t1` artan sırada.
    pub points: [Point; POINTS],
}

impl Curve {
    /// Verilen sıcaklıkta tablonun söylediği duty — **BASAMAK**, interpolasyon
    /// değil (nedeni modül başlığında).
    ///
    /// `temp_c`'nin geçtiği son satırın `duty`'sini döner. İlk eşiğin altında
    /// [`None`]: tablo orası için bir şey söylemiyor, ve "0" demek uydurmak
    /// olurdu — başlık satırının duty'si turbo'da 0 değil 63.
    pub fn duty_step(&self, temp_c: u8) -> Option<u8> {
        let mut out = None;
        for pt in &self.points {
            if temp_c >= pt.t1_c {
                out = Some(pt.duty_pct);
            } else {
                break; // t1 artan; ilk geçilmeyen satırdan sonrası da geçilmez
            }
        }
        out
    }

    /// Fanın kımıldadığı ilk sıcaklık (`points[0].t1_c`).
    pub fn first_threshold_c(&self) -> u8 {
        self.points[0].t1_c
    }

    /// Tablodaki en yüksek duty.
    pub fn max_duty_pct(&self) -> u8 {
        let mut m = 0;
        let mut i = 0;
        while i < POINTS {
            if self.points[i].duty_pct > m {
                m = self.points[i].duty_pct;
            }
            i += 1;
        }
        m
    }
}

/// Bir mod + fan için eğri. Beş modun ikisi de tabloda var, `Option` yok.
pub fn curve(mode: FanMode, fan: Fan) -> &'static Curve {
    match (mode, fan) {
        (FanMode::Quiet, Fan::Fan0) => &QUIET_FAN0,
        (FanMode::Quiet, Fan::Fan1) => &QUIET_FAN1,
        (FanMode::Responsive, Fan::Fan0) => &RESPONSIVE_FAN0,
        (FanMode::Responsive, Fan::Fan1) => &RESPONSIVE_FAN1,
        (FanMode::Balanced, Fan::Fan0) => &BALANCED_FAN0,
        (FanMode::Balanced, Fan::Fan1) => &BALANCED_FAN1,
        (FanMode::Gaming, Fan::Fan0) => &GAMING_FAN0,
        (FanMode::Gaming, Fan::Fan1) => &GAMING_FAN1,
        (FanMode::Turbo, Fan::Fan0) => &TURBO_FAN0,
        (FanMode::Turbo, Fan::Fan1) => &TURBO_FAN1,
    }
}

/// On eğrinin tamamı (5 mod × 2 fan) — testler ve toplu çizim için.
pub fn all() -> &'static [&'static Curve] {
    &ALL_CURVES
}

static ALL_CURVES: [&Curve; 10] = [
    &QUIET_FAN0,
    &QUIET_FAN1,
    &RESPONSIVE_FAN0,
    &RESPONSIVE_FAN1,
    &BALANCED_FAN0,
    &BALANCED_FAN1,
    &GAMING_FAN0,
    &GAMING_FAN1,
    &TURBO_FAN0,
    &TURBO_FAN1,
];

/// `quiet` / fan 0 — firmware ofseti `0x05A66`
/// (grup tabanı `0x05A66` + `0` = `0x4B` × 0).
static QUIET_FAN0: Curve = Curve {
    mode: FanMode::Quiet,
    fan: Fan::Fan0,
    src_offset: 0x05A66,
    rule_mod: 0x02,
    rule_b0: 0x00,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 66, 0),
    points: [
        p( 54,  69, 18),   // r1   bayrak2=0x00
        p( 61,  71, 18),   // r2   bayrak2=0x00
        p( 63,  74, 20),   // r3   bayrak2=0x00
        p( 66,  76, 20),   // r4   bayrak2=0x00
        p( 68,  79, 21),   // r5   bayrak2=0x00
        p( 71,  81, 21),   // r6   bayrak2=0x00
        p( 73,  84, 23),   // r7   bayrak2=0x00
        p( 76,  87, 23),   // r8   bayrak2=0x00
        p( 79,  90, 23),   // r9   bayrak2=0x00
        p( 82,  92, 23),   // r10  bayrak2=0x00
        p( 84,  94, 23),   // r11  bayrak2=0x00
        p( 86,  96, 23),   // r12  bayrak2=0x00
        p( 88,  98, 26),   // r13  bayrak2=0x10
        p( 90, 100, 29),   // r14  bayrak2=0x01
    ],
};

/// `quiet` / fan 1 — firmware ofseti `0x05AB1`
/// (grup tabanı `0x05A66` + `75` = `0x4B` × 1).
static QUIET_FAN1: Curve = Curve {
    mode: FanMode::Quiet,
    fan: Fan::Fan1,
    src_offset: 0x05AB1,
    rule_mod: 0x02,
    rule_b0: 0x10,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 56, 0),
    points: [
        p( 50,  60, 18),   // r1   bayrak2=0x00
        p( 54,  64, 21),   // r2   bayrak2=0x00
        p( 58,  68, 21),   // r3   bayrak2=0x10
        p( 62,  71, 23),   // r4   bayrak2=0x21
        p( 65,  72, 26),   // r5   bayrak2=0x02
        p( 68,  75, 29),   // r6   bayrak2=0x02
        p( 70,  78, 29),   // r7   bayrak2=0x02
        p( 73,  81, 29),   // r8   bayrak2=0x02
        p( 76,  84, 29),   // r9   bayrak2=0x02
        p( 79,  87, 29),   // r10  bayrak2=0x02
        p( 82,  90, 29),   // r11  bayrak2=0x02
        p( 85,  93, 29),   // r12  bayrak2=0x02
        p( 88,  96, 29),   // r13  bayrak2=0x02
        p( 91, 100, 29),   // r14  bayrak2=0x02
    ],
};

/// `responsive` / fan 0 — firmware ofseti `0x05B92`
/// (grup tabanı `0x05B92` + `0` = `0x4B` × 0).
static RESPONSIVE_FAN0: Curve = Curve {
    mode: FanMode::Responsive,
    fan: Fan::Fan0,
    src_offset: 0x05B92,
    rule_mod: 0x00,
    rule_b0: 0x00,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 46, 0),
    points: [
        p( 40,  54, 18),   // r1   bayrak2=0x00
        p( 46,  59, 20),   // r2   bayrak2=0x00
        p( 51,  64, 21),   // r3   bayrak2=0x00
        p( 56,  68, 23),   // r4   bayrak2=0x00
        p( 60,  72, 26),   // r5   bayrak2=0x00
        p( 64,  75, 29),   // r6   bayrak2=0x00
        p( 67,  81, 33),   // r7   bayrak2=0x00
        p( 76,  84, 33),   // r8   bayrak2=0x00
        p( 79,  87, 33),   // r9   bayrak2=0x00
        p( 82,  90, 33),   // r10  bayrak2=0x00
        p( 85,  93, 33),   // r11  bayrak2=0x00
        p( 88,  96, 33),   // r12  bayrak2=0x00
        p( 91,  98, 38),   // r13  bayrak2=0x10
        p( 93, 100, 43),   // r14  bayrak2=0x01
    ],
};

/// `responsive` / fan 1 — firmware ofseti `0x05BDD`
/// (grup tabanı `0x05B92` + `75` = `0x4B` × 1).
static RESPONSIVE_FAN1: Curve = Curve {
    mode: FanMode::Responsive,
    fan: Fan::Fan1,
    src_offset: 0x05BDD,
    rule_mod: 0x00,
    rule_b0: 0x10,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 54, 0),
    points: [
        p( 48,  57, 18),   // r1   bayrak2=0x00
        p( 51,  60, 20),   // r2   bayrak2=0x00
        p( 54,  62, 21),   // r3   bayrak2=0x00
        p( 56,  64, 23),   // r4   bayrak2=0x00
        p( 58,  66, 26),   // r5   bayrak2=0x00
        p( 60,  68, 29),   // r6   bayrak2=0x10
        p( 62,  70, 33),   // r7   bayrak2=0x21
        p( 64,  74, 38),   // r8   bayrak2=0x02
        p( 68,  82, 43),   // r9   bayrak2=0x02
        p( 77,  86, 43),   // r10  bayrak2=0x02
        p( 81,  90, 43),   // r11  bayrak2=0x02
        p( 85,  94, 43),   // r12  bayrak2=0x02
        p( 89,  98, 43),   // r13  bayrak2=0x02
        p( 93, 100, 43),   // r14  bayrak2=0x02
    ],
};

/// `balanced` / fan 0 — firmware ofseti `0x05CBE`
/// (grup tabanı `0x05CBE` + `0` = `0x4B` × 0).
static BALANCED_FAN0: Curve = Curve {
    mode: FanMode::Balanced,
    fan: Fan::Fan0,
    src_offset: 0x05CBE,
    rule_mod: 0x04,
    rule_b0: 0x00,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 66, 0),
    points: [
        p( 54,  70, 18),   // r1   bayrak2=0x00
        p( 60,  74, 20),   // r2   bayrak2=0x00
        p( 66,  77, 21),   // r3   bayrak2=0x00
        p( 69,  80, 23),   // r4   bayrak2=0x00
        p( 72,  83, 26),   // r5   bayrak2=0x00
        p( 75,  86, 29),   // r6   bayrak2=0x00
        p( 78,  88, 33),   // r7   bayrak2=0x00
        p( 80,  90, 33),   // r8   bayrak2=0x00
        p( 82,  92, 33),   // r9   bayrak2=0x00
        p( 84,  94, 33),   // r10  bayrak2=0x00
        p( 86,  95, 33),   // r11  bayrak2=0x00
        p( 87,  96, 33),   // r12  bayrak2=0x00
        p( 88,  98, 38),   // r13  bayrak2=0x10
        p( 90, 100, 43),   // r14  bayrak2=0x01
    ],
};

/// `balanced` / fan 1 — firmware ofseti `0x05D09`
/// (grup tabanı `0x05CBE` + `75` = `0x4B` × 1).
static BALANCED_FAN1: Curve = Curve {
    mode: FanMode::Balanced,
    fan: Fan::Fan1,
    src_offset: 0x05D09,
    rule_mod: 0x04,
    rule_b0: 0x10,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 54, 0),
    points: [
        p( 48,  57, 18),   // r1   bayrak2=0x00
        p( 51,  60, 20),   // r2   bayrak2=0x00
        p( 54,  62, 21),   // r3   bayrak2=0x00
        p( 56,  64, 23),   // r4   bayrak2=0x00
        p( 58,  66, 26),   // r5   bayrak2=0x00
        p( 60,  68, 29),   // r6   bayrak2=0x10
        p( 62,  70, 33),   // r7   bayrak2=0x21
        p( 64,  74, 38),   // r8   bayrak2=0x02
        p( 68,  82, 43),   // r9   bayrak2=0x02
        p( 77,  86, 43),   // r10  bayrak2=0x02
        p( 81,  90, 43),   // r11  bayrak2=0x02
        p( 85,  94, 43),   // r12  bayrak2=0x02
        p( 89,  98, 43),   // r13  bayrak2=0x02
        p( 93, 100, 43),   // r14  bayrak2=0x02
    ],
};

/// `gaming` / fan 0 — firmware ofseti `0x05DEA`
/// (grup tabanı `0x05DEA` + `0` = `0x4B` × 0).
static GAMING_FAN0: Curve = Curve {
    mode: FanMode::Gaming,
    fan: Fan::Fan0,
    src_offset: 0x05DEA,
    rule_mod: 0x01,
    rule_b0: 0x00,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 46, 0),
    points: [
        p( 40,  54, 18),   // r1   bayrak2=0x00
        p( 46,  59, 20),   // r2   bayrak2=0x00
        p( 51,  64, 21),   // r3   bayrak2=0x00
        p( 56,  68, 23),   // r4   bayrak2=0x00
        p( 60,  72, 26),   // r5   bayrak2=0x00
        p( 64,  75, 29),   // r6   bayrak2=0x00
        p( 67,  80, 33),   // r7   bayrak2=0x00
        p( 72,  84, 33),   // r8   bayrak2=0x00
        p( 76,  88, 33),   // r9   bayrak2=0x00
        p( 80,  92, 33),   // r10  bayrak2=0x00
        p( 84,  96, 38),   // r11  bayrak2=0x00
        p( 86,  97, 43),   // r12  bayrak2=0x00
        p( 88, 105, 48),   // r13  bayrak2=0x10
        p( 90, 110, 53),   // r14  bayrak2=0x01
    ],
};

/// `gaming` / fan 1 — firmware ofseti `0x05E35`
/// (grup tabanı `0x05DEA` + `75` = `0x4B` × 1).
static GAMING_FAN1: Curve = Curve {
    mode: FanMode::Gaming,
    fan: Fan::Fan1,
    src_offset: 0x05E35,
    rule_mod: 0x01,
    rule_b0: 0x10,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 51, 0),
    points: [
        p( 46,  54, 18),   // r1   bayrak2=0x00
        p( 49,  57, 20),   // r2   bayrak2=0x00
        p( 52,  60, 21),   // r3   bayrak2=0x00
        p( 55,  63, 23),   // r4   bayrak2=0x00
        p( 58,  66, 26),   // r5   bayrak2=0x00
        p( 61,  68, 29),   // r6   bayrak2=0x00
        p( 63,  70, 33),   // r7   bayrak2=0x00
        p( 65,  72, 38),   // r8   bayrak2=0x00
        p( 67,  74, 43),   // r9   bayrak2=0x00
        p( 69,  76, 48),   // r10  bayrak2=0x10
        p( 71,  80, 53),   // r11  bayrak2=0x21
        p( 75,  84, 53),   // r12  bayrak2=0x02
        p( 79,  88, 53),   // r13  bayrak2=0x02
        p( 83, 100, 53),   // r14  bayrak2=0x02
    ],
};

/// `turbo` / fan 0 — firmware ofseti `0x05F16`
/// (grup tabanı `0x05F16` + `0` = `0x4B` × 0).
static TURBO_FAN0: Curve = Curve {
    mode: FanMode::Turbo,
    fan: Fan::Fan0,
    src_offset: 0x05F16,
    rule_mod: 0x03,
    rule_b0: 0x00,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 42, 63),
    points: [
        p( 36,  48, 63),   // r1   bayrak2=0x00
        p( 43,  54, 63),   // r2   bayrak2=0x00
        p( 49,  60, 63),   // r3   bayrak2=0x00
        p( 55,  66, 63),   // r4   bayrak2=0x00
        p( 61,  72, 63),   // r5   bayrak2=0x00
        p( 67,  76, 63),   // r6   bayrak2=0x00
        p( 71,  80, 63),   // r7   bayrak2=0x00
        p( 75,  84, 63),   // r8   bayrak2=0x00
        p( 79,  88, 63),   // r9   bayrak2=0x00
        p( 83,  92, 63),   // r10  bayrak2=0x00
        p( 87,  94, 63),   // r11  bayrak2=0x00
        p( 89,  96, 63),   // r12  bayrak2=0x00
        p( 91, 105, 63),   // r13  bayrak2=0x10
        p(100, 110, 63),   // r14  bayrak2=0x01
    ],
};

/// `turbo` / fan 1 — firmware ofseti `0x05F61`
/// (grup tabanı `0x05F16` + `75` = `0x4B` × 1).
static TURBO_FAN1: Curve = Curve {
    mode: FanMode::Turbo,
    fan: Fan::Fan1,
    src_offset: 0x05F61,
    rule_mod: 0x03,
    rule_b0: 0x10,
    // XRAM'e KOPYALANMAYAN başlık satırı (t1 = 0). Yorumu ölçülmedi.
    header: p(0, 51, 63),
    points: [
        p( 46,  54, 63),   // r1   bayrak2=0x00
        p( 49,  57, 63),   // r2   bayrak2=0x00
        p( 52,  60, 63),   // r3   bayrak2=0x00
        p( 55,  63, 63),   // r4   bayrak2=0x00
        p( 58,  66, 63),   // r5   bayrak2=0x00
        p( 61,  68, 63),   // r6   bayrak2=0x00
        p( 63,  70, 63),   // r7   bayrak2=0x00
        p( 65,  72, 63),   // r8   bayrak2=0x00
        p( 67,  74, 63),   // r9   bayrak2=0x00
        p( 71,  76, 63),   // r10  bayrak2=0x10
        p( 75,  80, 63),   // r11  bayrak2=0x21
        p( 79,  84, 63),   // r12  bayrak2=0x02
        p( 83,  88, 63),   // r13  bayrak2=0x02
        p( 87, 100, 63),   // r14  bayrak2=0x02
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    const MODES: [FanMode; 5] = [
        FanMode::Quiet,
        FanMode::Balanced,
        FanMode::Responsive,
        FanMode::Gaming,
        FanMode::Turbo,
    ];

    #[test]
    fn her_tablo_on_dort_nokta() {
        // 15 satır × 5 bayt dosyada; XRAM'e kopyalanan 14. Bu sayı canlı
        // ölçümle (0xF8AC'ten okunan 14×3) birebir tutmalı.
        for c in all() {
            assert_eq!(c.points.len(), 14, "{:?} {:?}", c.mode, c.fan);
        }
        assert_eq!(all().len(), 10, "5 mod × 2 fan");
    }

    #[test]
    fn duty_yuzde_araliginda() {
        for c in all() {
            for (i, pt) in c.points.iter().enumerate() {
                assert!(
                    pt.duty_pct <= 100,
                    "{:?} {:?} r{}: duty={}",
                    c.mode, c.fan, i + 1, pt.duty_pct
                );
            }
        }
    }

    #[test]
    fn sicakliklar_kesin_artan() {
        // duty_step'in erken çıkışı buna dayanıyor — bozulursa lookup bozulur.
        for c in all() {
            for w in c.points.windows(2) {
                assert!(
                    w[1].t1_c > w[0].t1_c,
                    "{:?} {:?}: t1 artmıyor {} -> {}",
                    c.mode, c.fan, w[0].t1_c, w[1].t1_c
                );
            }
        }
    }

    #[test]
    fn duty_azalmiyor() {
        for c in all() {
            for w in c.points.windows(2) {
                assert!(
                    w[1].duty_pct >= w[0].duty_pct,
                    "{:?} {:?}: duty düşüyor {} -> {}",
                    c.mode, c.fan, w[0].duty_pct, w[1].duty_pct
                );
            }
        }
    }

    /// BİLİNEN TABLO, BİLİNEN SATIR. `0x05A66` (sessiz / fan 0) 7 Eyl 2026'da
    /// canlı olarak EC XRAM'de görüldü; ilk ve son satırı elle sabitliyoruz ki
    /// veri bir daha üretilirse kayma fark edilsin.
    #[test]
    fn bilinen_tablonun_bilinen_satiri() {
        let c = curve(FanMode::Quiet, Fan::Fan0);
        assert_eq!(c.src_offset, 0x05A66);
        assert_eq!(c.points[0], p(54, 69, 18), "r1");
        assert_eq!(c.points[13], p(90, 100, 29), "r14");

        // Fan 1 tablosu grup tabanı + 0x4B (canlı ölçümde 0x05AB1'di).
        let c1 = curve(FanMode::Quiet, Fan::Fan1);
        assert_eq!(c1.src_offset, 0x05AB1);
        assert_eq!(c1.points[0], p(50, 60, 18), "r1");
        assert_eq!(c1.points[13], p(91, 100, 29), "r14");

        // Mod 4 = "balanced": makine 7 Eyl'de bu eğriyle koşuyordu.
        assert_eq!(curve(FanMode::Balanced, Fan::Fan0).src_offset, 0x05CBE);
        assert_eq!(curve(FanMode::Balanced, Fan::Fan0).rule_mod, 0x04);
    }

    /// Turbo düz: on dört satırın hepsi %63. `describe()`'daki "düz %63" bu.
    #[test]
    fn turbo_duz_63() {
        for fan in [Fan::Fan0, Fan::Fan1] {
            let c = curve(FanMode::Turbo, fan);
            assert!(
                c.points.iter().all(|pt| pt.duty_pct == 63),
                "{:?} turbo düz değil",
                fan
            );
        }
        assert_eq!(curve(FanMode::Turbo, Fan::Fan0).first_threshold_c(), 36);
    }

    /// Grup ofsetleri kural tablosundaki adreslerle ve `0x4B` adımıyla tutuyor.
    #[test]
    fn ofsetler_kural_tablosuyla_tutar() {
        for m in MODES {
            let f0 = curve(m, Fan::Fan0);
            let f1 = curve(m, Fan::Fan1);
            assert_eq!(f1.src_offset, f0.src_offset + TABLE_STRIDE, "{m:?}");
            assert_eq!(f0.rule_b0, 0x00, "{m:?}");
            assert_eq!(f1.rule_b0, 0x10, "{m:?}");
            assert_eq!(f0.rule_mod, f1.rule_mod, "{m:?}");
        }
        // Beş modun iç mod numarası birbirinden farklı olmalı.
        let mut mods: Vec<u8> = MODES.iter().map(|&m| curve(m, Fan::Fan0).rule_mod).collect();
        mods.sort_unstable();
        mods.dedup();
        assert_eq!(mods.len(), 5, "iki mod aynı kural kaydına bakıyor");
    }

    /// `FanMode::describe()` metni tablodan türetilmişti; ikisi ayrışmasın.
    #[test]
    fn curve_ozetler_fan_mode_describe_ile_tutar() {
        // (mod, ilk eşik °C, iki fandaki en yüksek duty)
        let beklenen = [
            (FanMode::Quiet, 54u8, 29u8),
            (FanMode::Balanced, 54, 43),
            (FanMode::Responsive, 40, 43),
            (FanMode::Gaming, 40, 53),
            (FanMode::Turbo, 36, 63),
        ];
        for (m, esik, tavan) in beklenen {
            let c0 = curve(m, Fan::Fan0);
            let c1 = curve(m, Fan::Fan1);
            assert_eq!(c0.first_threshold_c(), esik, "{m:?} ilk eşik");
            assert_eq!(
                c0.max_duty_pct().max(c1.max_duty_pct()),
                tavan,
                "{m:?} duty tavanı"
            );
            let d = m.describe();
            assert!(
                d.contains(&esik.to_string()) && d.contains(&tavan.to_string()),
                "describe() = {d:?} ile tablo ayrıştı ({m:?})"
            );
        }
    }

    #[test]
    fn duty_step_basamak_interpolasyon_degil() {
        let c = curve(FanMode::Quiet, Fan::Fan0);
        // r1 = (54, 69, 18), r2 = (61, 71, 18), r3 = (63, 74, 20)
        assert_eq!(c.duty_step(53), None, "ilk eşiğin altı: tablo susuyor");
        assert_eq!(c.duty_step(54), Some(18), "eşik dahil");
        // 62, r2 ile r3 arasında. İnterpolasyon 19 derdi; basamak r2'yi verir.
        assert_eq!(c.duty_step(62), Some(18), "ARA DEĞER İNTERPOLE EDİLMEZ");
        assert_eq!(c.duty_step(63), Some(20));
        assert_eq!(c.duty_step(90), Some(29), "son satır");
        assert_eq!(c.duty_step(255), Some(29), "tablonun üstü son satırda kalır");
    }

    #[test]
    fn duty_step_monoton_ve_tabloda_var_olan_bir_deger() {
        for c in all() {
            let mut onceki = 0u8;
            for t in 0..=120u8 {
                match c.duty_step(t) {
                    None => assert!(t < c.first_threshold_c()),
                    Some(v) => {
                        assert!(v >= onceki, "{:?} {:?} {t} °C'de düştü", c.mode, c.fan);
                        assert!(
                            c.points.iter().any(|pt| pt.duty_pct == v),
                            "{v} tabloda yok — ara değer üretilmiş"
                        );
                        onceki = v;
                    }
                }
            }
        }
    }

    /// Başlık satırı taşınıyor ama hesaba girmiyor — turbo'da duty'si 0 değil,
    /// yani "başlık = fan kapalı" varsayımı yanlış olurdu.
    #[test]
    fn baslik_satiri_hesaba_girmiyor() {
        for c in all() {
            assert_eq!(c.header.t1_c, 0, "başlık t1 = 0 olmalı");
        }
        assert_eq!(curve(FanMode::Turbo, Fan::Fan0).header.duty_pct, 63);
        assert_eq!(curve(FanMode::Quiet, Fan::Fan0).header.duty_pct, 0);
        // Yine de 36 °C altında turbo için bir şey İDDİA ETMİYORUZ.
        assert_eq!(curve(FanMode::Turbo, Fan::Fan0).duty_step(20), None);
    }
}
