// SPDX-License-Identifier: GPL-2.0-only
//! AERO Kontrol — Gigabyte AERO X16 1VH (EG61VH) kontrol arayüzü.
//!
//! # Yetki
//!
//! Okuma yolu tamamen yetkisiz: okuduğu değerlerin hepsi dünyaya-okunur
//! (ölçüldü, 7 Eyl 2026). Yazma yolu `aero_sysfs::write` üzerinden var olan
//! köprüleri çağırıyor (`PLAN.md` §10 adım 6/8) — uygulama root istemiyor,
//! polkit kuralı `sched.nix`'teki gibi noktasal.
//!
//! # Tema
//!
//! Kendi renk paleti YOK. `libcosmic`, `~/.config/cosmic/com.system76.CosmicTheme.*`
//! dosyalarını canlı okuyor — vurgu rengi, gece/gündüz, yoğunluk, font, ikon
//! teması bedava geliyor ve COSMIC Ayarlar'dan değiştirince anında uyuyor.
//! Bu depoda `~/.config/cosmic`'e **yazmak yasak**; yalnız okunuyor.
//!
//! # Boşta güç
//!
//! Örnekleme yalnız uygulama açıkken ve görünür panele göre yapılıyor
//! (Durum/Fan'da 2 sn, diğerlerinde 5 sn). Uygulama kapanınca hiçbir şey
//! arkada kalmıyor — 4.28 W boşta bütçesi kuralı.

mod egri;

use aero_sysfs::{Action, FanMode, Snapshot, apply, curves};
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{Alignment, Length, Subscription, time};
use cosmic::widget::{self, Canvas, segmented_button};
use cosmic::{Application, Apply, Element, executor};
use egri::{Cizgi, Grafik};
use std::time::Duration;

const APP_ID: &str = "dev.zixar.AeroControl";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    OnAyarlar,
    Durum,
    FanTermal,
    FanEgrisi,
    GucPerformans,
    Pil,
    Hakkinda,
}

/// Basit menünün "tavsiye edilen ayar" paketleri (`PLAN.md` §5).
///
/// Her biri fan modu + performans profilini TUTARLI bir paket olarak kuruyor.
/// Amacı bir şeyi bozamamak: ham sayı yok, isimlendirilmiş kombinasyon var.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OnAyar {
    ad: &'static str,
    aciklama: &'static str,
    fan: FanMode,
    profil: &'static str,
}

/// `profil` değerleri sürücünün sunduğu üçlüden: low-power / balanced /
/// performance. (`balanced-performance` ölçümle düşürüldü — fazladan seçenek
/// `amd-pmf`'e de yazılıyordu ve orada ne yaptığı ölçülmemişti.)
const ON_AYARLAR: &[OnAyar] = &[
    OnAyar {
        ad: "Sessiz",
        aciklama: "Okuma, yazma, pil ömrü. Fan 54 °C'ye kadar durur.",
        fan: FanMode::Quiet,
        profil: "low-power",
    },
    OnAyar {
        ad: "Dengeli",
        aciklama: "Günlük kullanım. Geç başlar ama gerekince yükselir — varsayılan.",
        fan: FanMode::Balanced,
        profil: "balanced",
    },
    OnAyar {
        ad: "Duyarlı",
        aciklama: "Erken soğutma isteyen. Fan 40 °C'de devreye girer.",
        fan: FanMode::Responsive,
        profil: "balanced",
    },
    OnAyar {
        ad: "Performans",
        aciklama: "Oyun ve derleme. Daha yüksek güç bütçesi, daha sesli fan.",
        fan: FanMode::Gaming,
        profil: "performance",
    },
    OnAyar {
        ad: "Maksimum",
        aciklama: "Kısa süreli tam yük. Fan düz %63 — sürekli kullanım için değil.",
        fan: FanMode::Turbo,
        profil: "performance",
    },
];

/// Eğri panelinde gösterilecek modun kaynağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EgriSecim {
    /// Makinede etkin olan mod izleniyor — kullanıcı bir şey seçmedi.
    Etkin,
    /// Kullanıcı bir modu sabitledi; artık `Tik` bunu geri almıyor.
    Sabit(FanMode),
}

/// Eğri panelindeki mod listesi. Sıra sürücünün `fan_mode_choices`
/// çıktısıyla aynı — iki yerde iki farklı sıra kafa karıştırır.
const EGRI_MODLARI: [FanMode; 5] = [
    FanMode::Quiet,
    FanMode::Balanced,
    FanMode::Responsive,
    FanMode::Gaming,
    FanMode::Turbo,
];

#[derive(Debug, Clone)]
enum Message {
    /// Zamanlayıcı tetikledi — sysfs'i yeniden oku.
    Tik,
    /// Bir ön ayar paketini uygula (fan modu + profil).
    OnAyarUygula(usize),
    /// Tek bir fan modunu uygula.
    FanModu(FanMode),
    /// Tek bir profili uygula.
    Profil(String),
    /// Şarj limiti kaydırıcısı sürükleniyor (henüz yazma yok).
    SarjKaydir(u8),
    /// Kaydırıcı bırakıldı — şimdi yaz.
    SarjUygula,
    /// Hata şeridini kapat.
    HataKapat,

    /// Eğri panelinde gösterilecek mod (0 = etkin modu izle).
    EgriMod(usize),
    /// "Beş modu birden" anahtarı.
    EgriKarsilastir(bool),
    /// Karşılaştırma kipinde hangi fanın eğrileri çizilsin.
    EgriFan(usize),
    /// Ham tablo ve kaynak kanıtı bölümü.
    EgriHam(bool),
}

struct App {
    core: Core,
    nav: segmented_button::SingleSelectModel,
    snap: Snapshot,
    /// Kaydırıcı sürüklenirken geçici değer; bırakılınca yazılıyor.
    sarj_taslak: u8,
    /// Son yazma hatası — kullanıcıya aynen gösteriliyor, yutulmuyor.
    hata: Option<String>,

    /// Eğri panelinin görünüm durumu. Hiçbiri donanıma dokunmuyor: bu dört
    /// alan yalnız neyin ÇİZİLECEĞİNİ belirliyor, neyin AYARLANACAĞINI değil.
    egri_secim: EgriSecim,
    egri_karsilastir: bool,
    egri_fan: u8,
    egri_ham: bool,
}

/// Gömülü nav simgesi.
///
/// Bu üç panelin simgesi kurulu ikon temasında (Pop/Cosmic) **yok** — 8 Eyl
/// 2026'da tek tek bakıldı: `temperature-symbolic`,
/// `power-profile-performance-symbolic` ve herhangi bir çizgi-grafiği adı
/// (`office-chart-line-symbolic`, `x-office-chart-symbolic` …) çözülmüyor ve
/// nav'da o satırlar boş kalıyordu. Simgeler bu yüzden depoda ve ikiliye
/// gömülü: tema ne olursa olsun çiziliyorlar.
///
/// Diğer üç panel (`Ön Ayarlar`, `Pil`, `Hakkında`, `Durum`) temadan çözülen
/// standart adları kullanmaya devam ediyor — orada gömmeye gerek yok.
fn gomulu_simge(svg: &'static [u8]) -> widget::icon::Icon {
    widget::icon::from_svg_bytes(svg).symbolic(true).icon()
}

/// Seçili modun iki fan tablosunu yan yana, tek monospace blokta.
///
/// Kaynak ofseti ve kural kaydı da burada. Bu, "bu eğri nereden geliyor"
/// sorusunun cevabının bir ÇIKARIM değil bir ALINTI olduğu yer: kaydı üreteç
/// firmware imajından aynen okuyor (`aero-sysfs/gen-curves.py`), yeniden
/// kurmuyor.
fn ham_tablo(m: FanMode) -> String {
    let (Some(a), Some(b)) = (curves::egri(m, 0), curves::egri(m, 1)) else {
        return "Bu mod için tablo yok.".into();
    };
    let onaltilik = |k: &[u8; 8]| {
        k.iter()
            .map(|x| format!("{x:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let mut o = String::with_capacity(1024);
    o.push_str(&format!(
        "EC iç mod  0x{:02X}\nKural tablosu  0x{:05X}\n\n",
        a.ic_mod(),
        curves::KURAL_TABLOSU
    ));
    for (ad, e) in [("Fan 1", a), ("Fan 2", b)] {
        o.push_str(&format!(
            "{ad}  kaynak 0x{:05X}  kural {}\n",
            e.kaynak,
            onaltilik(&e.kural)
        ));
    }
    o.push_str(&format!("\n{:5}{:^16}  {:^16}\n", "", "Fan 1", "Fan 2"));
    o.push_str(&format!(
        "{:>3}  {:>5}{:>5}{:>6}  {:>5}{:>5}{:>6}\n",
        "#", "t1", "t2", "duty", "t1", "t2", "duty"
    ));
    for (i, (x, y)) in a.noktalar.iter().zip(b.noktalar.iter()).enumerate() {
        o.push_str(&format!(
            "{:>3}  {:>5}{:>5}{:>6}  {:>5}{:>5}{:>6}\n",
            i + 1,
            x.t1,
            x.t2,
            x.duty,
            y.t1,
            y.t2,
            y.duty
        ));
    }
    o.push_str(
        "\nKural kaydı düzeni: { b0, 00, b2, iç_mod, 00, FF, adres_hi, adres_lo }\n\
         b0 grup içindeki tabloyu seçiyor: 00 = fan 0, 10 = fan 1.\n\
         b0 = 30/40 tabloları HİÇ görülmedi (bütün ölçümler pilde yapıldı).\n\
         t2 sütununun hangi sensöre baktığı ölçülmedi; grafik yalnız t1'i kullanıyor.",
    );
    o
}

impl App {
    fn panel(&self) -> Panel {
        self.nav
            .active_data::<Panel>()
            .copied()
            .unwrap_or(Panel::Durum)
    }

    /// Eksik yetenekleri anlatan üst şerit. `aero-sysfs`'in sözleşmesinin
    /// görünür yüzü: sürücü yoksa çökmüyoruz, ne yapılacağını söylüyoruz.
    fn eksikler(&self) -> Option<Element<'_, Message>> {
        if self.snap.problems.is_empty() {
            return None;
        }

        let mut col = widget::column::with_capacity(self.snap.problems.len()).spacing(4);
        for p in &self.snap.problems {
            col =
                col.push(widget::text::body(format!("{} — {}", p.what, p.why)).width(Length::Fill));
        }

        Some(
            widget::container(
                widget::column::with_capacity(8)
                    .spacing(8)
                    .push(widget::text::title4("Eksik yetenekler"))
                    .push(col),
            )
            .class(cosmic::theme::Container::Card)
            .padding(16)
            .width(Length::Fill)
            .into(),
        )
    }

    fn satir<'a>(&self, ad: &'a str, deger: String) -> Element<'a, Message> {
        widget::settings::item::builder(ad)
            .control(widget::text::body(deger))
            .into()
    }

    fn yok() -> String {
        "—".into()
    }

    /// Eylemi köprüye gönderir ve durumu HEMEN geri okur.
    /// Yazdığımızı değil sistemin okuduğunu göstermek bu uygulamanın
    /// varlık sebebi — `aorus_laptop`'ın hatası tam olarak yazdığını
    /// bildirmekti.
    fn uygula(&mut self, a: Action) {
        match apply(&a) {
            Ok(()) => self.hata = None,
            Err(e) => self.hata = Some(e.to_string()),
        }
        self.snap = Snapshot::read();
        if let Some(v) = self.snap.charge_limit_pct {
            self.sarj_taslak = v;
        }
    }

    /// Son yazma hatası — yutmuyoruz, aynen gösteriyoruz.
    fn hata_seridi(&self) -> Option<Element<'_, Message>> {
        let h = self.hata.as_ref()?;
        Some(
            widget::container(
                widget::row::with_capacity(2)
                    .spacing(12)
                    .align_y(Alignment::Center)
                    .push(widget::text::body(h.clone()).width(Length::Fill))
                    .push(widget::button::text("Kapat").on_press(Message::HataKapat)),
            )
            .class(cosmic::theme::Container::Card)
            .padding(16)
            .width(Length::Fill)
            .into(),
        )
    }

    /// Basit menü: beş isimlendirilmiş paket + şarj limiti. Ham sayı yok.
    fn on_ayarlar(&self) -> Element<'_, Message> {
        let s = &self.snap;
        let mut sec = widget::settings::section()
            .title("Ön ayarlar")
            .add(widget::text::caption(
                "Her ön ayar fan modu ile performans profilini tutarlı bir paket \
                 olarak kurar. Ayrı ayrı ayarlamak için diğer panellere bakın.",
            ));

        for (i, o) in ON_AYARLAR.iter().enumerate() {
            let etkin =
                s.fan_mode == Some(o.fan) && s.platform_profile.as_deref() == Some(o.profil);
            let dugme = if etkin {
                widget::button::suggested("Etkin")
            } else {
                widget::button::standard("Uygula").on_press(Message::OnAyarUygula(i))
            };
            sec = sec.add(
                widget::settings::item::builder(o.ad)
                    .description(o.aciklama)
                    .control(dugme),
            );
        }

        let sarj = widget::settings::section().title("Şarj limiti").add(
            widget::settings::item::builder(format!("%{}", self.sarj_taslak))
                .description("Pil ömrü için 60 tavsiye edilir; yolculuk öncesi 100 yapın.")
                .control(
                    widget::slider(1..=100u8, self.sarj_taslak, Message::SarjKaydir)
                        .on_release(Message::SarjUygula)
                        .width(Length::Fixed(240.0)),
                ),
        );

        widget::column::with_capacity(4)
            .spacing(24)
            .push(sec)
            .push(sarj)
            .into()
    }

    fn durum(&self) -> Element<'_, Message> {
        let s = &self.snap;
        widget::settings::section()
            .title("Canlı")
            .add(self.satir(
                "CPU sıcaklığı",
                s.cpu_temp_c.map_or_else(Self::yok, |v| format!("{v} °C")),
            ))
            .add(self.satir(
                "Fan 1",
                s.fan1_rpm.map_or_else(Self::yok, |v| format!("{v} rpm")),
            ))
            .add(self.satir(
                "Fan 2",
                s.fan2_rpm.map_or_else(Self::yok, |v| format!("{v} rpm")),
            ))
            .add(self.satir(
                "Güç kaynağı",
                match s.ac_online {
                    Some(true) => "Fişte".into(),
                    Some(false) => "Pilde".into(),
                    None => Self::yok(),
                },
            ))
            .add(self.satir(
                "Fan modu",
                s.fan_mode.map_or_else(
                    || {
                        s.fan_mode_raw_unknown
                            .clone()
                            .map_or_else(Self::yok, |r| format!("tanınmıyor ({r})"))
                    },
                    |m| m.label().to_string(),
                ),
            ))
            .add(self.satir(
                "Performans profili",
                s.platform_profile.clone().unwrap_or_else(Self::yok),
            ))
            .add(self.satir(
                "Pil",
                s.battery_pct.map_or_else(Self::yok, |v| format!("%{v}")),
            ))
            .into()
    }

    fn fan_termal(&self) -> Element<'_, Message> {
        let s = &self.snap;

        let mut modlar = widget::settings::section().title("Fan modu");
        if s.fan_mode_choices.is_empty() {
            modlar = modlar.add(widget::text::body(
                "Sürücü mod listesi vermiyor — yüklü değil ya da eski bir sürüm.",
            ));
        } else {
            for m in &s.fan_mode_choices {
                let secili = s.fan_mode == Some(*m);
                let dugme = if secili {
                    widget::button::suggested("Etkin")
                } else {
                    widget::button::standard("Seç").on_press(Message::FanModu(*m))
                };
                modlar = modlar.add(
                    widget::settings::item::builder(m.label())
                        .description(m.describe())
                        .control(dugme),
                );
            }
        }

        widget::column::with_capacity(8)
            .spacing(24)
            .push(
                widget::settings::section()
                    .title("Canlı")
                    .add(self.satir(
                        "CPU sıcaklığı",
                        s.cpu_temp_c.map_or_else(Self::yok, |v| format!("{v} °C")),
                    ))
                    .add(self.satir(
                        "Fan 1",
                        s.fan1_rpm.map_or_else(Self::yok, |v| format!("{v} rpm")),
                    ))
                    .add(self.satir(
                        "Fan 2",
                        s.fan2_rpm.map_or_else(Self::yok, |v| format!("{v} rpm")),
                    )),
            )
            .push(modlar)
            .push(widget::text::caption(
                "Fan hızı bu firmware'de doğrudan ayarlanamıyor; yalnız hangi \
                 eğrinin kullanılacağı seçilebiliyor. (Ölçüldü: duty yazmaçları \
                 yazımı kabul ediyor ama fan umursamıyor.)",
            ))
            .push(widget::text::caption(
                "Bir modun eğrisini ŞEKİL olarak görmek için «Fan Eğrisi» paneline bakın.",
            ))
            .into()
    }

    /// Fan eğrisi paneli — `PLAN.md` §4.1.
    ///
    /// Veri firmware imajından geliyor (`aero_sysfs::curves`), yani bu panel
    /// açıkken donanıma **fazladan tek bir okuma** yapılmıyor: canlı imleç de
    /// zaten toplanan anlık görüntüyü kullanıyor.
    ///
    /// Panelde OLMAYAN iki şey ve gerekçeleri ekranda da yazıyor: "EC'den
    /// doğrula" düğmesi (EIDR okumasının zaman aşımı görülemiyor) ve eğri
    /// düzenleme (veri portu salt okunur, ölçüldü).
    fn fan_egrisi(&self) -> Element<'_, Message> {
        let s = &self.snap;

        let gosterilen = match self.egri_secim {
            EgriSecim::Etkin => s.fan_mode,
            EgriSecim::Sabit(m) => Some(m),
        };
        // Etkin mod okunamıyorsa tek bir eğri çizmek uydurma olurdu; beş modu
        // birden göstermek hem doğru hem daha faydalı.
        let mod_bilinmiyor = gosterilen.is_none();
        let karsilastir = self.egri_karsilastir || mod_bilinmiyor;

        let cizgiler: Vec<Cizgi> = match (karsilastir, gosterilen) {
            // Tek mod: aynı modun iki fanı, aynı renkte, fan 2 kesik.
            (false, Some(m)) => (0..2u8)
                .filter_map(|fan| {
                    curves::egri(m, fan).map(|e| Cizgi {
                        egri: e,
                        mod_: m,
                        kesik: fan == 1,
                        etiket: format!("Fan {}", fan + 1),
                    })
                })
                .collect(),
            // Karşılaştırma: beş mod, tek fan, mod başına bir renk.
            _ => EGRI_MODLARI
                .iter()
                .filter_map(|m| {
                    curves::egri(*m, self.egri_fan).map(|e| Cizgi {
                        egri: e,
                        mod_: *m,
                        kesik: self.egri_fan == 1,
                        etiket: m.label().to_string(),
                    })
                })
                .collect(),
        };

        let grafik = widget::container(
            Canvas::new(Grafik {
                cizgiler,
                cpu: s.cpu_temp_c,
            })
            .width(Length::Fill)
            .height(Length::Fixed(320.0)),
        )
        .class(cosmic::theme::Container::Card)
        .padding(12)
        .width(Length::Fill);

        // --- görünüm kontrolleri: hiçbiri makinede bir şey DEĞİŞTİRMİYOR ---
        let mut secenekler: Vec<String> = Vec::with_capacity(6);
        secenekler.push(match s.fan_mode {
            Some(m) => format!("Etkin mod — {}", m.label()),
            None => "Etkin mod — okunamıyor".into(),
        });
        secenekler.extend(EGRI_MODLARI.iter().map(|m| m.label().to_string()));

        let secili = match self.egri_secim {
            EgriSecim::Etkin => 0,
            EgriSecim::Sabit(m) => 1 + EGRI_MODLARI.iter().position(|x| *x == m).unwrap_or(0),
        };

        let mut gorunum = widget::settings::section()
            .title("Görünüm")
            .add(
                widget::settings::item::builder("Gösterilen eğri")
                    .description("Yalnız çizimi değiştirir — makinede hiçbir ayar yapmaz.")
                    .control(widget::dropdown(secenekler, Some(secili), Message::EgriMod)),
            )
            .add(
                widget::settings::item::builder("Beş modu birden")
                    .description("Mod seçiminin ne değiştirdiğini tek bakışta gösterir.")
                    .control(
                        widget::toggler(self.egri_karsilastir).on_toggle(Message::EgriKarsilastir),
                    ),
            );

        if karsilastir {
            gorunum = gorunum.add(
                widget::settings::item::builder("Hangi fan")
                    .description("Karşılaştırmada tek fan çiziliyor — iki fanın tabloları farklı.")
                    .control(widget::dropdown(
                        vec!["Fan 1".to_string(), "Fan 2".to_string()],
                        Some(self.egri_fan as usize),
                        Message::EgriFan,
                    )),
            );
        }

        gorunum = gorunum.add(
            widget::settings::item::builder("Ham tablo ve kaynak")
                .description("14 satır × 3 sütun, kaynak ofseti, kural tablosu kaydı.")
                .control(widget::toggler(self.egri_ham).on_toggle(Message::EgriHam)),
        );

        // --- canlı okuma: grafiğin DIŞINDA, sayı olarak (PLAN §4.1) ---
        let mut canli = widget::settings::section()
            .title("Canlı")
            .add(self.satir(
                "CPU sıcaklığı",
                s.cpu_temp_c.map_or_else(Self::yok, |v| format!("{v} °C")),
            ))
            .add(self.satir(
                "Fan 1",
                s.fan1_rpm.map_or_else(Self::yok, |v| format!("{v} rpm")),
            ))
            .add(self.satir(
                "Fan 2",
                s.fan2_rpm.map_or_else(Self::yok, |v| format!("{v} rpm")),
            ));

        // Beklenen duty YALNIZ etkin mod çiziliyorken anlamlı: başka bir modun
        // eğrisine bakarken "beklenen" demek yanlış olurdu.
        if let (Some(t), Some(m), Some(etkin)) = (s.cpu_temp_c, gosterilen, s.fan_mode)
            && m == etkin
        {
            let d = |fan: u8| {
                curves::egri(m, fan).map_or_else(Self::yok, |e| format!("%{}", e.duty(t)))
            };
            canli = canli.add(self.satir(
                "Beklenen duty (Fan 1 / Fan 2)",
                format!("{} / {}", d(0), d(1)),
            ));
        }

        let mut col = widget::column::with_capacity(12).spacing(24);

        if mod_bilinmiyor {
            col = col.push(widget::text::body(
                "Etkin fan modu okunamadı, o yüzden beş mod birden çiziliyor — \
                 tek bir eğri seçmek uydurma olurdu.",
            ));
        }

        col = col.push(grafik);

        col = col.push(widget::text::caption(
            "Basamak çiziliyor, eğri değil: EŞİKLER ölçüldü, iki eşik ARASINDAKİ \
             davranış (interpolasyon mu, eşik mi) ölçülmedi. Grafikteki şekil, \
             uygulamanın hesabıyla birebir aynı — arada ikinci bir yorum yok.",
        ));

        col = col.push(widget::text::caption(
            "Dikey düz çizgi CPU sıcaklığı, kesik çizgi farenin gösterdiği yer. \
             Eğrinin sıcaklık sütununun (`t1`) hangi sensöre baktığı ÖLÇÜLMEDİ — \
             kesişimdeki duty bir okuma değil, bir tahmindir.",
        ));

        col = col.push(gorunum).push(canli);

        if self.egri_ham
            && let Some(m) = gosterilen
        {
            col = col.push(
                widget::settings::section()
                    .title(format!("Ham tablo — {}", m.label()))
                    .add(widget::text::monotext(ham_tablo(m))),
            );
        }

        col.push(widget::text::caption(
            "Bu firmware'de eğriler DEĞİŞTİRİLEMEZ; mod seçimi yalnız hangi \
             eğrinin yükleneceğini belirler. (Veri portu salt okunur — ölçüldü.)",
        ))
        .push(widget::text::caption(
            "«EC'den doğrula» düğmesi YOK: EC'deki canlı eğriyi okumak `EIDR` \
             mailbox'ını gerektiriyor, ama zaman aşımı bayrağını (`ECTE`) dışarı \
             veren bir ACPI yolu yok — bayat veriyi geçerli cevaptan ayıramadığımız \
             için okuma sunulmuyor (`kernel/README.md`). Buradaki tablolar firmware \
             imajından ve ölçülen sekiz aktif tablonun sekizi de kaynakla birebir \
             eşleşti.",
        ))
        .into()
    }

    fn guc_performans(&self) -> Element<'_, Message> {
        let s = &self.snap;
        let mut sec = widget::settings::section().title("Performans profili");

        sec = sec.add(self.satir(
            "Etkin",
            s.platform_profile.clone().unwrap_or_else(Self::yok),
        ));
        for p in &s.platform_profile_choices {
            let etkin = s.platform_profile.as_deref() == Some(p.as_str());
            let dugme = if etkin {
                widget::button::suggested("Etkin")
            } else {
                widget::button::standard("Seç").on_press(Message::Profil(p.clone()))
            };
            sec = sec.add(widget::settings::item::builder(p.clone()).control(dugme));
        }

        let mut col = widget::column::with_capacity(8).spacing(24).push(sec);

        if s.platform_profile.as_deref() == Some("custom") {
            col = col.push(widget::text::caption(
                "«custom» = sürücü henüz bir profil yazmadı. EC aktif profili geri \
                 vermiyor (WMBC'de 0xED okuması yok), o yüzden uydurulmuyor. \
                 İlk profil değişiminde gerçek değere oturur.",
            ));
        }

        col.push(widget::text::caption(
            "Profil, CPU güç limitleri ile dGPU boost bütçesini tek pakette \
             anahtarlıyor. Fan modu buna DAHİL DEĞİL — ayrı bir kol, çünkü \
             güç yöneticisi profili fiş takıp çıkarınca yeniden yazıyor ve \
             seçtiğin fan modunu geri alması istenmiyor.",
        ))
        .into()
    }

    fn pil(&self) -> Element<'_, Message> {
        let s = &self.snap;
        widget::column::with_capacity(8)
            .spacing(24)
            .push(
                widget::settings::section()
                    .title("Pil")
                    .add(self.satir(
                        "Doluluk",
                        s.battery_pct.map_or_else(Self::yok, |v| format!("%{v}")),
                    ))
                    .add(
                        self.satir(
                            "Şarj limiti",
                            s.charge_limit_pct
                                .map_or_else(Self::yok, |v| format!("%{v}")),
                        ),
                    )
                    .add(self.satir(
                        "Güç kaynağı",
                        match s.ac_online {
                            Some(true) => "Fişte".into(),
                            Some(false) => "Pilde".into(),
                            None => Self::yok(),
                        },
                    )),
            )
            .push(widget::text::caption(
                "Şarj limiti standart çekirdek arayüzü üzerinden yazılıyor ve \
                 sürücü her yazımı geri okuyup doğruluyor. Uyanışta yeniden \
                 uygulanıyor.",
            ))
            .into()
    }

    fn hakkinda(&self) -> Element<'_, Message> {
        widget::column::with_capacity(8)
            .spacing(16)
            .push(widget::text::title3("AERO Kontrol"))
            .push(widget::text::body(
                "Gigabyte AERO X16 1VH (SKU EG61VH) için kontrol arayüzü.",
            ))
            .push(widget::text::caption(
                "Yalnız ölçülmüş yetenekler gösterilir. Bu makinede fan hızı \
                 ayarlanamıyor, soket sıcaklığı kanalı ölü, klavye aydınlatma \
                 yazmacı etkisiz — hiçbiri arayüzde yok, çünkü çalışmayan bir \
                 düğme koymak düğme koymamaktan kötüdür.",
            ))
            .push(widget::text::caption(format!(
                "Sürücü: {}",
                if self.snap.driver_present {
                    "aero_eg61h yüklü"
                } else {
                    "YÜKLÜ DEĞİL"
                }
            )))
            .into()
    }
}

impl Application for App {
    type Executor = executor::Default;
    type Flags = ();
    type Message = Message;

    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let mut nav = segmented_button::SingleSelectModel::default();
        nav.insert()
            .text("Ön Ayarlar")
            .icon(widget::icon::from_name("preferences-system-symbolic"))
            .data(Panel::OnAyarlar)
            .activate();
        nav.insert()
            .text("Durum")
            .icon(widget::icon::from_name("utilities-system-monitor-symbolic"))
            .data(Panel::Durum);
        nav.insert()
            .text("Fan ve Termal")
            .icon(gomulu_simge(include_bytes!(
                "../data/icons/fan-symbolic.svg"
            )))
            .data(Panel::FanTermal);
        nav.insert()
            .text("Fan Eğrisi")
            .icon(gomulu_simge(include_bytes!(
                "../data/icons/fan-curve-symbolic.svg"
            )))
            .data(Panel::FanEgrisi);
        nav.insert()
            .text("Güç ve Performans")
            .icon(gomulu_simge(include_bytes!(
                "../data/icons/power-symbolic.svg"
            )))
            .data(Panel::GucPerformans);
        nav.insert()
            .text("Pil")
            .icon(widget::icon::from_name("battery-symbolic"))
            .data(Panel::Pil);
        nav.insert()
            .text("Hakkında")
            .icon(widget::icon::from_name("help-about-symbolic"))
            .data(Panel::Hakkinda);

        let snap = Snapshot::read();
        let app = App {
            core,
            nav,
            sarj_taslak: snap.charge_limit_pct.unwrap_or(60),
            snap,
            hata: None,
            egri_secim: EgriSecim::Etkin,
            egri_karsilastir: false,
            egri_fan: 0,
            egri_ham: false,
        };

        (app, Task::none())
    }

    fn nav_model(&self) -> Option<&segmented_button::SingleSelectModel> {
        Some(&self.nav)
    }

    fn on_nav_select(&mut self, entity: segmented_button::Entity) -> Task<Self::Message> {
        self.nav.activate(entity);
        Task::none()
    }

    fn update(&mut self, message: Self::Message) -> Task<Self::Message> {
        match message {
            Message::Tik => {
                self.snap = Snapshot::read();
                // Kaydırıcı sürüklenmiyorken sistemin gerçek değerini izle.
                if let Some(v) = self.snap.charge_limit_pct {
                    self.sarj_taslak = v;
                }
            }

            Message::HataKapat => self.hata = None,

            // Eğri paneli: dördü de yalnız ÇİZİMİ değiştiriyor, donanıma
            // hiçbir yazma yapmıyor.
            Message::EgriMod(i) => {
                self.egri_secim = match i.checked_sub(1).and_then(|k| EGRI_MODLARI.get(k)) {
                    Some(m) => EgriSecim::Sabit(*m),
                    // 0 ya da beklenmedik indeks: etkin modu izlemeye dön.
                    None => EgriSecim::Etkin,
                };
            }

            Message::EgriKarsilastir(v) => self.egri_karsilastir = v,

            // Sürücü iki fan sunuyor; başka bir indeks gelirse fan 1'e düş.
            Message::EgriFan(i) => self.egri_fan = u8::from(i == 1),

            Message::EgriHam(v) => self.egri_ham = v,

            Message::SarjKaydir(v) => self.sarj_taslak = v,

            Message::SarjUygula => self.uygula(Action::ChargeLimit(self.sarj_taslak)),

            Message::FanModu(m) => self.uygula(Action::FanMode(m)),

            Message::Profil(p) => self.uygula(Action::Profile(p)),

            Message::OnAyarUygula(i) => {
                if let Some(o) = ON_AYARLAR.get(i) {
                    // Fan modu ÖNCE: profil yazımı PPD üzerinden gidiyor ve
                    // AC/pil olaylarını tetikleyebiliyor; fanı önce oturtmak
                    // ara durumda yanlış eğride kalmayı kısaltıyor.
                    self.uygula(Action::FanMode(o.fan));
                    if self.hata.is_none() {
                        self.uygula(Action::Profile(o.profil.to_string()));
                    }
                }
            }
        }
        Task::none()
    }

    /// Örnekleme YALNIZ uygulama açıkken. Görünür panele göre yavaşlıyor:
    /// canlı sayı göstermeyen panellerde EC'yi boşuna yormanın anlamı yok.
    fn subscription(&self) -> Subscription<Self::Message> {
        let period = match self.panel() {
            // Eğri panelinde canlı imleç var — Durum/Fan ile aynı hızda.
            Panel::Durum | Panel::FanTermal | Panel::FanEgrisi => Duration::from_secs(2),
            Panel::OnAyarlar => Duration::from_secs(3),
            _ => Duration::from_secs(5),
        };
        time::every(period).map(|_| Message::Tik)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icerik = match self.panel() {
            Panel::OnAyarlar => self.on_ayarlar(),
            Panel::Durum => self.durum(),
            Panel::FanTermal => self.fan_termal(),
            Panel::FanEgrisi => self.fan_egrisi(),
            Panel::GucPerformans => self.guc_performans(),
            Panel::Pil => self.pil(),
            Panel::Hakkinda => self.hakkinda(),
        };

        let mut col = widget::column::with_capacity(8).spacing(24);
        if let Some(h) = self.hata_seridi() {
            col = col.push(h);
        }
        if let Some(banner) = self.eksikler() {
            col = col.push(banner);
        }
        col = col.push(icerik);

        col.apply(widget::container)
            .width(Length::Fill)
            .align_x(Alignment::Center)
            .apply(widget::scrollable)
            .height(Length::Fill)
            .into()
    }
}

fn main() -> cosmic::iced::Result {
    cosmic::app::run::<App>(
        Settings::default().size_limits(
            cosmic::iced::Limits::NONE
                .min_width(420.0)
                .min_height(320.0),
        ),
        (),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ham tablo bölümü KANIT sunuyor: kaynak ofseti ve kural kaydı. Biçim
    /// bozulursa kanıt okunamaz hâle gelir, o yüzden burada sabitleniyor.
    #[test]
    fn ham_tablo_kaniti_tasiyor() {
        let t = ham_tablo(FanMode::Balanced);

        // Tam 14 veri satırı, sırasıyla 1..14 ve her biri 7 sayı
        // (# + iki fanın t1/t2/duty'si). Sütunlar kayarsa kanıt okunmaz.
        let veri: Vec<Vec<u32>> = t
            .lines()
            .filter_map(|l| {
                let s: Option<Vec<u32>> = l.split_whitespace().map(|x| x.parse().ok()).collect();
                s.filter(|v| v.len() == 7)
            })
            .collect();
        assert_eq!(veri.len(), 14, "14 veri satırı bekleniyordu:\n{t}");
        for (i, satir) in veri.iter().enumerate() {
            assert_eq!(satir[0] as usize, i + 1, "satır numarası atladı:\n{t}");
        }
        // İlk satır Dengeli'nin ölçülen ilk noktası: fan 0 54/70/18, fan 1 48/57/18.
        assert_eq!(veri[0], vec![1, 54, 70, 18, 48, 57, 18], "\n{t}");
        assert_eq!(veri[13], vec![14, 90, 100, 43, 93, 100, 43], "\n{t}");

        // Kanıt: iki fanın kaynak ofseti ve iç mod.
        assert!(t.contains("0x05CBE"), "fan 0 kaynak ofseti yok:\n{t}");
        assert!(t.contains("0x05D09"), "fan 1 kaynak ofseti yok:\n{t}");
        assert!(t.contains("0x04"), "iç mod yok:\n{t}");
        assert!(t.contains("0x0616E"), "kural tablosu adresi yok:\n{t}");
        // Kural kaydı imajdan aynen: fan 1'inki 10 00 10 04 00 FF 5D 09.
        assert!(
            t.contains("10 00 10 04 00 FF 5D 09"),
            "kural kaydı yok:\n{t}"
        );
    }

    /// Beş modun beşi de tablo üretebilmeli — "Bu mod için tablo yok" düşüşü
    /// kullanıcıya boş bir bölüm gösterirdi.
    #[test]
    fn bes_modun_hepsi_tablo_uretiyor() {
        for m in EGRI_MODLARI {
            let t = ham_tablo(m);
            assert!(!t.contains("tablo yok"), "{m:?}");
            assert!(t.contains("Fan 1") && t.contains("Fan 2"), "{m:?}");
        }
    }
}
