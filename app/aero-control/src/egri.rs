// SPDX-License-Identifier: GPL-2.0-only
//! Fan eğrisi grafiği — `duty = f(sıcaklık)`, **basamak** çizimi.
//!
//! # Neden basamak, neden eğri değil
//!
//! `t1`/`t2` eşiklerinin ara değerlerdeki davranışı (interpolasyon mu, eşik mi)
//! **ölçülmedi** (`~/ecscope/docs/firmware-8051.md` §9). Basamak, en az iddia
//! eden gösterim: burada çizilen şekil [`aero_sysfs::curves::Egri::duty`]'nin
//! döndürdüğü değerin ta kendisi — grafik ile hesap arasında ikinci bir yorum
//! katmanı yok. Ölçüm interpolasyon çıkarırsa ikisi birden değişir.
//!
//! # Renk sözlüğü
//!
//! **Renk = mod, kesik çizgi = fan 2.** Tek mod görünümünde aynı modun iki fanı
//! aynı renkte ama biri kesik; karşılaştırma kipinde beş mod beş renkte ve
//! kesiklik seçili fana göre. Böylece rengin anlamı iki görünüm arasında
//! değişmiyor.
//!
//! Renkler COSMIC temasından geliyor (`accent`/`success`/`warning`/`destructive`/
//! `on_bg`) — uygulamanın kendi paleti YOK, `PLAN.md` §6.
//!
//! # Efsane neden tuvalin içinde
//!
//! Ölçülen en yüksek duty %63 (Turbo), Y ekseni ise 0-100 — yani grafiğin üst
//! üçte biri her modda boş. Efsane oraya sabitlenince ne eğrileri örtüyor ne de
//! imleç kıpırdadıkça yer değiştiriyor.
//!
//! # Yetki ve güç
//!
//! Bu dosya donanıma **hiç dokunmuyor**: veri firmware imajından çıkarılmış
//! sabit tablo (`aero_sysfs::curves`). Panel açıkken fazladan tek bir EC okuması
//! yapılmıyor; canlı imleç zaten toplanan anlık görüntüyü kullanıyor.

use aero_sysfs::{FanMode, curves::Egri};
use cosmic::Theme;
use cosmic::iced::advanced::text::Alignment as YaziHiza;
use cosmic::iced::alignment;
use cosmic::iced::{Color, Point, Rectangle, Size, mouse};
use cosmic::widget::canvas::{self, Frame, Geometry, LineDash, Path, Stroke, Text};

/// Grafik kenar boşlukları (piksel). Sol, Y etiketleri (`%100`) için;
/// alt, X etiketleri ve imleç yazısı için.
const SOL: f32 = 40.0;
const SAG: f32 = 14.0;
const UST: f32 = 12.0;
const ALT: f32 = 26.0;

/// Fan 2'nin kesik çizgi deseni.
const KESIK: [f32; 2] = [6.0, 4.0];

/// Fare imlecinin deseni — canlı imleçten (düz) ayırt edilsin diye.
const IMLEC_KESIK: [f32; 2] = [4.0, 3.0];

const YAZI: f32 = 11.0;

/// İki alt eksen yazısının merkezleri bundan yakınsa biri gizleniyor.
/// "50" ile "51 °C"nin yarım genişlikleri toplamı kadar.
const ETIKET_ARALIK: f32 = 26.0;

/// Sağa yaslı son tik etiketinin ("100 °C") yarım genişliği.
const SON_ETIKET_YARIM: f32 = 22.0;

/// Efsanedeki çizgi örneğinin uzunluğu ve yazıya kalan boşluk.
const ORNEK_BOY: f32 = 20.0;
const ORNEK_BOSLUK: f32 = 7.0;

/// X ekseninin sağ ucu. **Tjmax = 100 °C** (Zen5 mobil) — üstünde okuma
/// beklenmiyor, ve 100 °C zaten bütün tabloların son eşiği ya da üstü.
const T_MAX: f32 = 100.0;

/// X ekseninin varsayılan sol ucu. En düşük eşik 36 °C (Turbo, fan 0), yani
/// bütün eğrilerin "fan durur" bölgesi görünür kalıyor.
const T_MIN: f32 = 30.0;

/// Çizilecek tek eğri.
pub struct Cizgi {
    pub egri: &'static Egri,
    /// Renk anahtarı — moddan geliyor.
    pub mod_: FanMode,
    /// `true` ise kesik çizgi (fan 2).
    pub kesik: bool,
    /// Efsane etiketi ("Fan 1", "Dengeli" …).
    pub etiket: String,
}

/// Bir modun rengi. **Tek yer** — hem tuval hem dışarıdaki metinler buradan
/// okuyor, yoksa ikisi birbirinden habersiz kayar.
pub fn mod_rengi(tema: &Theme, m: FanMode) -> Color {
    let c = tema.cosmic();
    match m {
        // Sessiz: nötr — hiçbir şey iddia etmeyen mod.
        FanMode::Quiet => c.on_bg_color().into(),
        // Dengeli: vurgu rengi — önerilen/varsayılan olan.
        FanMode::Balanced => c.accent_color().into(),
        FanMode::Responsive => c.success_color().into(),
        FanMode::Gaming => c.warning_color().into(),
        // Maksimum: uygulamanın kendi tarifi "sürekli kullanım için değil".
        FanMode::Turbo => c.destructive_color().into(),
    }
}

fn alfa(c: Color, a: f32) -> Color {
    Color { a, ..c }
}

fn desen<'a>(kesik: bool, d: &'a [f32]) -> LineDash<'a> {
    if kesik {
        LineDash {
            segments: d,
            offset: 0,
        }
    } else {
        LineDash::default()
    }
}

/// Basamak yolunun **veri uzayındaki** köşeleri: `(sıcaklık, duty)`.
///
/// Tuvalden ayrı durmasının tek sebebi sınanabilirlik. Bu panelin bütün
/// iddiası "grafikte gördüğün şekil, uygulamanın hesabının ta kendisi" —
/// ve o iddiayı bir test ([`tests::cizim_hesapla_ayni`]) bu dizi üzerinden
/// doğruluyor: her tam sayı sıcaklıkta poligonun değeri [`Egri::duty`] ile
/// birebir eşleşmek zorunda.
///
/// Eşikte **dikey sıçrama**, arada **yatay** — interpolasyon yok.
fn basamak_noktalari(egri: &Egri, tmin: f32, tmax: f32) -> Vec<(f32, f32)> {
    let mut v = Vec::with_capacity(2 * egri.noktalar.len() + 2);
    let mut d = f32::from(egri.duty(tmin.max(0.0) as u8));
    v.push((tmin, d));
    for n in &egri.noktalar {
        let t = f32::from(n.t1);
        if t <= tmin {
            continue;
        }
        if t > tmax {
            break;
        }
        v.push((t, d));
        v.push((t, f32::from(n.duty)));
        d = f32::from(n.duty);
    }
    v.push((tmax, d));
    v
}

/// Eksen dönüşümü. `draw` sırasında kuruluyor, çünkü çizim alanı ancak o zaman
/// biliniyor.
struct Eksen {
    alan: Rectangle,
    tmin: f32,
    tmax: f32,
}

impl Eksen {
    fn px(&self, t: f32) -> f32 {
        self.alan.x + (t - self.tmin) / (self.tmax - self.tmin) * self.alan.width
    }

    fn py(&self, duty: f32) -> f32 {
        self.alan.y + (1.0 - duty / 100.0) * self.alan.height
    }

    /// Piksel → sıcaklık. Fare imleci için.
    fn t(&self, x: f32) -> f32 {
        self.tmin + (x - self.alan.x) / self.alan.width * (self.tmax - self.tmin)
    }
}

/// Grafiğin tuval programı. Her `view()` çağrısında yeniden kuruluyor; içinde
/// kalıcı durum yok. [`FareDurumu`] yalnız "yeniden çiz" kararı için var.
pub struct Grafik {
    pub cizgiler: Vec<Cizgi>,
    /// Canlı CPU sıcaklığı. `None` ise canlı imleç çizilmiyor.
    pub cpu: Option<u8>,
}

/// Bir dikey imlecin görünümü.
struct ImlecBicimi {
    renk: Color,
    /// Kesik çizgi mi (fare imleci) yoksa düz mü (canlı CPU).
    kesik: bool,
    /// Alt eksene sıcaklık yazısı düşsün mü — iki imleç yan yanaysa biri susar.
    etiket: bool,
}

/// Fare imlecinin gösterdiği sıcaklık. Yalnız değişimi yakalamak için
/// tutuluyor; çizim `draw`'a gelen `cursor`'dan hesaplıyor.
#[derive(Default)]
pub struct FareDurumu {
    son: Option<u8>,
}

impl Grafik {
    /// X ekseninin sol ucu: normalde 30 °C, ama canlı sıcaklık daha düşükse
    /// imleç ekran dışında kalmasın diye aşağı iniyor.
    fn tmin(&self) -> f32 {
        match self.cpu {
            Some(t) if (t as f32) < T_MIN + 2.0 => (t as f32 - 4.0).max(0.0),
            _ => T_MIN,
        }
    }

    fn eksen(&self, boyut: Size) -> Eksen {
        Eksen {
            alan: Rectangle {
                x: SOL,
                y: UST,
                width: (boyut.width - SOL - SAG).max(1.0),
                height: (boyut.height - UST - ALT).max(1.0),
            },
            tmin: self.tmin(),
            tmax: T_MAX,
        }
    }

    /// Fare hangi sıcaklığın üstünde? Çizim alanının dışındaysa `None`.
    fn fare_sicakligi(&self, sinir: Rectangle, fare: mouse::Cursor) -> Option<u8> {
        let p = fare.position_in(sinir)?;
        let e = self.eksen(sinir.size());
        if p.x < e.alan.x || p.x > e.alan.x + e.alan.width {
            return None;
        }
        if p.y < e.alan.y || p.y > e.alan.y + e.alan.height {
            return None;
        }
        Some(e.t(p.x).round().clamp(0.0, 255.0) as u8)
    }

    /// Bir eğrinin basamak yolu — [`basamak_noktalari`]'nı eksene taşır.
    fn basamak(&self, e: &Eksen, egri: &Egri) -> Path {
        let v = basamak_noktalari(egri, e.tmin, e.tmax);
        Path::new(|b| {
            let mut ilk = true;
            for (t, d) in v {
                let p = Point::new(e.px(t), e.py(d));
                if ilk {
                    b.move_to(p);
                    ilk = false;
                } else {
                    b.line_to(p);
                }
            }
        })
    }

    /// Izgara ve eksen etiketleri.
    ///
    /// `imlecler`, çizilecek dikey imleçlerin x konumları. Bir tik etiketi
    /// imleç yazısıyla çakışacaksa **çizilmiyor**: 51 °C'de duran imlecin
    /// yazısı "50" tikinin üstüne biner ve ikisi de okunmaz hâle gelirdi
    /// (8 Eyl'de ekranda görüldü).
    fn izgara(&self, f: &mut Frame, e: &Eksen, on: Color, imlecler: &[f32]) {
        // Çizim alanının kendisi — kartın içinde nerede olduğu belli olsun.
        f.fill_rectangle(
            Point::new(e.alan.x, e.alan.y),
            Size::new(e.alan.width, e.alan.height),
            alfa(on, 0.04),
        );

        // Yatay: duty %0/25/50/75/100.
        for d in [0u8, 25, 50, 75, 100] {
            let y = e.py(d as f32);
            f.stroke(
                &Path::line(
                    Point::new(e.alan.x, y),
                    Point::new(e.alan.x + e.alan.width, y),
                ),
                Stroke::default()
                    .with_color(alfa(on, if d == 0 { 0.35 } else { 0.12 }))
                    .with_width(1.0),
            );
            f.fill_text(Text {
                content: format!("%{d}"),
                position: Point::new(e.alan.x - 6.0, y),
                color: alfa(on, 0.65),
                size: YAZI.into(),
                align_x: YaziHiza::Right,
                align_y: alignment::Vertical::Center,
                ..Text::default()
            });
        }

        // Dikey: 10 °C'de bir. Birim AYRI bir yazı değil, son tikin parçası —
        // ayrı yazıldığında sağ kenarda "100" ile üst üste biniyordu.
        let mut t = (e.tmin / 10.0).ceil() * 10.0;
        while t <= e.tmax + 0.1 {
            let x = e.px(t);
            f.stroke(
                &Path::line(
                    Point::new(x, e.alan.y),
                    Point::new(x, e.alan.y + e.alan.height),
                ),
                Stroke::default().with_color(alfa(on, 0.09)).with_width(1.0),
            );

            let son = t + 10.0 > e.tmax + 0.1;
            // Sağa yaslı son etiketin yatay ORTASI kendi ızgara çizgisinde
            // değil, solunda — çakışma testi buna göre.
            let orta = if son { x - SON_ETIKET_YARIM } else { x };
            if !imlecler.iter().any(|ix| (ix - orta).abs() < ETIKET_ARALIK) {
                f.fill_text(Text {
                    content: if son {
                        format!("{t:.0} °C")
                    } else {
                        format!("{t:.0}")
                    },
                    position: Point::new(x, e.alan.y + e.alan.height + 5.0),
                    color: alfa(on, 0.6),
                    size: YAZI.into(),
                    align_x: if son {
                        YaziHiza::Right
                    } else {
                        YaziHiza::Center
                    },
                    align_y: alignment::Vertical::Top,
                    ..Text::default()
                });
            }
            t += 10.0;
        }
    }

    /// Dikey imleç + her eğriyle kesiştiği noktalar + alt eksende sıcaklık
    /// etiketi.
    fn imlec(&self, f: &mut Frame, e: &Eksen, tema: &Theme, t: u8, b: ImlecBicimi) {
        let x = e.px(t as f32);
        if x < e.alan.x - 0.5 || x > e.alan.x + e.alan.width + 0.5 {
            return;
        }
        f.stroke(
            &Path::line(
                Point::new(x, e.alan.y),
                Point::new(x, e.alan.y + e.alan.height),
            ),
            Stroke {
                line_dash: desen(b.kesik, &IMLEC_KESIK),
                ..Stroke::default().with_color(b.renk).with_width(1.5)
            },
        );

        for c in &self.cizgiler {
            f.fill(
                &Path::circle(Point::new(x, e.py(c.egri.duty(t) as f32)), 3.0),
                mod_rengi(tema, c.mod_),
            );
        }

        if b.etiket {
            f.fill_text(Text {
                content: format!("{t} °C"),
                position: Point::new(
                    x.clamp(e.alan.x + 18.0, e.alan.x + e.alan.width - 18.0),
                    e.alan.y + e.alan.height + 5.0,
                ),
                color: b.renk,
                size: YAZI.into(),
                align_x: YaziHiza::Center,
                align_y: alignment::Vertical::Top,
                ..Text::default()
            });
        }
    }

    /// Efsane — her satır: çizgi örneği + etiket, ve sıcaklık biliniyorsa
    /// o sıcaklıktaki duty. Sol üstte sabit: duty tavanı %63 olduğu için
    /// grafiğin üst üçte biri her modda boş.
    fn efsane(&self, f: &mut Frame, e: &Eksen, tema: &Theme, t: Option<u8>) {
        let x = e.alan.x + 10.0;
        let mut y = e.alan.y + 8.0;

        for c in &self.cizgiler {
            let renk = mod_rengi(tema, c.mod_);
            let orta = y + YAZI / 2.0;
            f.stroke(
                &Path::line(Point::new(x, orta), Point::new(x + ORNEK_BOY, orta)),
                Stroke {
                    line_dash: desen(c.kesik, &KESIK),
                    ..Stroke::default().with_color(renk).with_width(2.0)
                },
            );
            f.fill_text(Text {
                content: match t {
                    Some(t) => format!("{} · %{}", c.etiket, c.egri.duty(t)),
                    None => c.etiket.clone(),
                },
                position: Point::new(x + ORNEK_BOY + ORNEK_BOSLUK, y),
                color: renk,
                size: YAZI.into(),
                align_x: YaziHiza::Left,
                align_y: alignment::Vertical::Top,
                ..Text::default()
            });
            y += YAZI + 4.0;
        }
    }
}

impl<Message> canvas::Program<Message, Theme> for Grafik {
    type State = FareDurumu;

    fn update(
        &self,
        durum: &mut Self::State,
        _olay: &canvas::Event,
        sinir: Rectangle,
        fare: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        // Yeniden çizimi YALNIZ değer değiştiğinde istiyoruz: fare her
        // kıpırdadığında tuvali yeniden çizmek boşta güç bütçesine yazık olur.
        let yeni = self.fare_sicakligi(sinir, fare);
        if yeni == durum.son {
            return None;
        }
        durum.son = yeni;
        Some(canvas::Action::request_redraw())
    }

    fn draw(
        &self,
        _durum: &Self::State,
        renderer: &cosmic::Renderer,
        tema: &Theme,
        sinir: Rectangle,
        fare: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, sinir.size());
        let e = self.eksen(sinir.size());
        let on = Color::from(tema.cosmic().on_bg_color());
        let uzerinde = self.fare_sicakligi(sinir, fare);

        // Izgara, hangi tik etiketinin gizleneceğini bilmek için imleçlerin
        // yerini önceden istiyor.
        let mut imlec_x: Vec<f32> = Vec::with_capacity(2);
        for t in [self.cpu, uzerinde].into_iter().flatten() {
            imlec_x.push(e.px(f32::from(t)));
        }
        self.izgara(&mut f, &e, on, &imlec_x);

        for c in &self.cizgiler {
            f.stroke(
                &self.basamak(&e, c.egri),
                Stroke {
                    line_dash: desen(c.kesik, &KESIK),
                    ..Stroke::default()
                        .with_color(mod_rengi(tema, c.mod_))
                        .with_width(2.0)
                },
            );
        }

        // Canlı CPU imleci düz; fare imleci kesik ve vurgu renginde.
        // İkisi birbirine çok yakınsa CANLI olanın yazısı düşüyor: fare
        // imleci kullanıcının o an baktığı yer, öncelik onun.
        if let Some(t) = self.cpu {
            let ayri = uzerinde
                .is_none_or(|h| (e.px(f32::from(t)) - e.px(f32::from(h))).abs() >= ETIKET_ARALIK);
            let b = ImlecBicimi {
                renk: alfa(on, 0.7),
                kesik: false,
                etiket: ayri,
            };
            self.imlec(&mut f, &e, tema, t, b);
        }
        if let Some(t) = uzerinde {
            let b = ImlecBicimi {
                renk: tema.cosmic().accent_color().into(),
                kesik: true,
                etiket: true,
            };
            self.imlec(&mut f, &e, tema, t, b);
        }

        // Fare varsa fareninki, yoksa canlı sıcaklığınki okunuyor.
        self.efsane(&mut f, &e, tema, uzerinde.or(self.cpu));

        vec![f.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        _durum: &Self::State,
        sinir: Rectangle,
        fare: mouse::Cursor,
    ) -> mouse::Interaction {
        if self.fare_sicakligi(sinir, fare).is_some() {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aero_sysfs::curves::EGRILER;

    /// Basamak poligonunun verilen sıcaklıktaki değeri: `x <= t` olan SON
    /// köşenin `y`'si. Aynı `x`'te iki köşe varsa (dikey sıçrama) sonuncusu
    /// kazanır — bu, `Egri::duty`'nin `t1 >= n.t1` davranışının aynısı.
    fn poligon_degeri(v: &[(f32, f32)], t: f32) -> f32 {
        let mut y = v[0].1;
        for (x, d) in v {
            if *x <= t {
                y = *d;
            } else {
                break;
            }
        }
        y
    }

    /// PANELİN TEK İDDİASI. Çizilen şekil ile hesaplanan duty aynı olmak
    /// zorunda; ayrılırlarsa grafik kullanıcıya yalan söylüyor demektir.
    #[test]
    fn cizim_hesapla_ayni() {
        // Üç farklı sol uç: varsayılan, imleç aşağı çektiğinde, ve ilk
        // eşiklerin ÜSTÜNDE bir uç (döngünün atlama dalını de kapsasın).
        for tmin in [T_MIN, 20.0, 70.0] {
            for e in &EGRILER {
                let v = basamak_noktalari(e, tmin, T_MAX);
                let mut t = tmin;
                while t <= T_MAX {
                    assert_eq!(
                        poligon_degeri(&v, t),
                        f32::from(e.duty(t as u8)),
                        "{:?} fan{} · tmin {tmin} · {t} °C",
                        e.mod_,
                        e.fan
                    );
                    t += 1.0;
                }
            }
        }
    }

    /// İnterpolasyon YOK: iki eşiğin arasında değer sabit kalmalı.
    /// (`curves.rs`'teki kardeş testin çizim tarafındaki karşılığı.)
    #[test]
    fn ara_degerde_interpolasyon_yok() {
        let e = aero_sysfs::curves::egri(FanMode::Quiet, 0).expect("sessiz fan 0");
        let v = basamak_noktalari(e, T_MIN, T_MAX);
        let a = e.noktalar[0];
        let b = e.noktalar[1];
        assert!(b.t1 > a.t1 + 1, "bu test iki eşiğin arasında yer ister");
        for t in (a.t1 + 1)..b.t1 {
            assert_eq!(
                poligon_degeri(&v, f32::from(t)),
                f32::from(a.duty),
                "{t} °C'de interpolasyon yapılıyor"
            );
        }
    }

    /// Poligon eksenin iki ucunu da kapatmalı — yoksa çizgi havada başlıyor.
    #[test]
    fn poligon_eksenin_iki_ucunu_kapatiyor() {
        for e in &EGRILER {
            let v = basamak_noktalari(e, T_MIN, T_MAX);
            assert_eq!(v.first().map(|p| p.0), Some(T_MIN), "{:?}", e.mod_);
            assert_eq!(v.last().map(|p| p.0), Some(T_MAX), "{:?}", e.mod_);
        }
    }

    fn eksen(tmin: f32) -> Eksen {
        Eksen {
            alan: Rectangle {
                x: SOL,
                y: UST,
                width: 400.0,
                height: 200.0,
            },
            tmin,
            tmax: T_MAX,
        }
    }

    #[test]
    fn eksen_uclari_ve_gidis_donus() {
        let e = eksen(T_MIN);
        assert_eq!(e.px(T_MIN), e.alan.x);
        assert_eq!(e.px(T_MAX), e.alan.x + e.alan.width);
        // duty %0 tabanda, %100 tavanda (ekran koordinatı aşağı doğru artar).
        assert_eq!(e.py(0.0), e.alan.y + e.alan.height);
        assert_eq!(e.py(100.0), e.alan.y);
        for t in [30.0, 42.0, 63.5, 100.0] {
            assert!(
                (e.t(e.px(t)) - t).abs() < 0.01,
                "{t} °C gidiş-dönüşte kaydı"
            );
        }
    }

    /// Canlı sıcaklık 30 °C'nin altına inerse imleç ekran dışında kalmamalı.
    #[test]
    fn dusuk_sicaklikta_eksen_asagi_iniyor() {
        let g = Grafik {
            cizgiler: Vec::new(),
            cpu: Some(26),
        };
        assert!(g.tmin() <= 26.0, "26 °C imleci eksenin dışında kalıyor");
        // Normal sıcaklıkta eksen oynamıyor — grafik sekmelerken zıplamasın.
        let g = Grafik {
            cizgiler: Vec::new(),
            cpu: Some(42),
        };
        assert_eq!(g.tmin(), T_MIN);
    }
}
