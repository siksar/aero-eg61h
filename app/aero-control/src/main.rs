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
//! (Status/Fan'da 2 sn, diğerlerinde 5 sn). Applyma kapanınca hiçbir şey
//! arkada kalmıyor — 4.28 W boşta bütçesi kuralı.

mod egri;
mod klavye;

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
    Status,
    FanTermal,
    FanEgrisi,
    GucPerformance,
    Battery,
    KlavyeIsik,
    Hakkinda,
}

/// Basit menünün "tavsiye edilen ayar" paketleri (`PLAN.md` §5).
///
/// Her biri fan mode + performans profilini TUTARLI bir paket olarak kuruyor.
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
        ad: "Quiet",
        aciklama: "Quiet operation and longer battery life. Fans start at 54 °C.",
        fan: FanMode::Quiet,
        profil: "low-power",
    },
    OnAyar {
        ad: "Balanced",
        aciklama: "Everyday use. Starts late and ramps up when needed — the default.",
        fan: FanMode::Balanced,
        profil: "balanced",
    },
    OnAyar {
        ad: "Responsive",
        aciklama: "Earlier cooling. Fans start at 40 °C.",
        fan: FanMode::Responsive,
        profil: "balanced",
    },
    OnAyar {
        ad: "Performance",
        aciklama: "Games and builds. Higher power budget and louder fans.",
        fan: FanMode::Gaming,
        profil: "performance",
    },
    OnAyar {
        ad: "Turbo",
        aciklama: "Short bursts of full load. Fans run at a flat 63% — not for continuous use.",
        fan: FanMode::Turbo,
        profil: "performance",
    },
];

/// Eğri panelinde gösterilecek modun kaynağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EgriSecim {
    /// Makinede etkin olan mod izleniyor — kullanıcı bir şey seçmedi.
    Active,
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
    /// Bir ön ayar paketini uygula (fan mode + profil).
    OnAyarApply(usize),
    /// Tek bir fan modenu uygula.
    FanModu(FanMode),
    /// Tek bir profili uygula.
    Profil(String),
    /// Charge limit kaydırıcısı sürükleniyor (henüz yazma yok).
    SarjKaydir(u8),
    /// Kaydırıcı bırakıldı — şimdi yaz.
    SarjApply,
    /// Hata şeridini kapat.
    HataDismiss,

    /// Eğri panelinde gösterilecek mod (0 = etkin modu izle).
    EgriMod(usize),
    /// "Compare all five modes" anahtarı.
    EgriKarsilastir(bool),
    /// Karşılaştırma kipinde hangi fanın eğrileri çizilsin.
    EgriFan(usize),
    /// Raw table and source kanıtı bölümü.
    EgriHam(bool),

    // --- Klavye aydınlatması --------------------------------------------
    /// Parlaklık kaydırıcısı sürükleniyor (henüz yazma yok).
    KbdParlaklikKaydir(u32),
    /// Kaydırıcı bırakıldı — şimdi yaz.
    KbdParlaklikApply,
    /// Hazır renk düğmesi ya da doğrulanmış hex girişi.
    KbdRenk(String),
    /// Hex metin kutusu düzenleniyor (henüz yazma yok).
    KbdHexDuzenle(String),
    /// Hex kutusunda Enter — doğrula ve yaz.
    KbdHexApply,
    /// Aç/kapa.
    KbdToggle,
    /// `Some(mod)` başlat/değiştir, `None` durdur.
    KbdAnimasyon(Option<String>),
    /// Stylix rengine dön (tema servisini yeniden koştur).
    KbdStylix,
}

struct App {
    core: Core,
    nav: segmented_button::SingleSelectModel,
    snap: Snapshot,
    /// Kaydırıcı sürüklenirken geçici değer; bırakılınca yazılıyor.
    sarj_taslak: u8,
    /// Kaydırıcı ŞU AN sürükleniyor mu. `Tik` bunu görüp taslağı ezmiyor —
    /// 12 Eyl 2026: eski kodun yorumu 'sürüklenmiyorken izle' diyordu ama koşul
    /// hiç yazılmamıştı, yani 2 sn'de bir gelen okuma kullanıcının parmağının
    /// altındaki değeri sistemin eski değerine geri zıplatıyordu.
    sarj_surukleniyor: bool,
    /// Son yazma hatası — kullanıcıya aynen gösteriliyor, yutulmuyor.
    hata: Option<String>,

    /// Eğri panelinin görünüm durumu. Hiçbiri donanıma dokunmuyor: bu dört
    /// alan yalnız neyin ÇİZİLECEĞİNİ belirliyor, neyin AYARLANACAĞINI değil.
    egri_secim: EgriSecim,
    egri_karsilastir: bool,
    egri_fan: u8,
    egri_ham: bool,

    /// Klavye ışığının okunmuş durumu. `Snapshot` ile BİRLİKTE tazeleniyor
    /// ama ondan ayrı: kaynağı sysfs değil, `kbd-rgb status --json`.
    kbd: klavye::Durum,
    /// Kaydırıcı sürüklenirken geçici parlaklık; şarj kaydırıcısıyla aynı
    /// desen — `Tik` sürükleme sırasında taslağı ezmiyor.
    kbd_parlaklik_taslak: u32,
    kbd_surukleniyor: bool,
    /// Hex metin kutusunun içeriği. Yazma yalnız Enter'da; her tuş vuruşunda
    /// donanıma gitmek hem gereksiz hem yarım hex'lerde hata üretirdi.
    kbd_hex: String,
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
/// Diğer üç panel (`Presets`, `Battery`, `About`, `Status`) temadan çözülen
/// standart adları kullanmaya devam ediyor — orada gömmeye gerek yok.
fn gomulu_simge(svg: &'static [u8]) -> widget::icon::Icon {
    widget::icon::from_svg_bytes(svg).symbolic(true).icon()
}

/// Selectili modun iki fan tablosunu yan yana, tek monospace blokta.
///
/// Kaynak ofseti ve kural kaydı da burada. Bu, "bu eğri nereden geliyor"
/// sorusunun cevabının bir ÇIKARIM değil bir ALINTI olduğu yer: kaydı üreteç
/// firmware imajından aynen okuyor (`aero-sysfs/gen-curves.py`), yeniden
/// kurmuyor.
fn ham_tablo(m: FanMode) -> String {
    let (Some(a), Some(b)) = (curves::egri(m, 0), curves::egri(m, 1)) else {
        return "No table is available for this mode.".into();
    };
    let onaltilik = |k: &[u8; 8]| {
        k.iter()
            .map(|x| format!("{x:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let mut o = String::with_capacity(1024);
    o.push_str(&format!(
        "EC internal mode  0x{:02X}\nRule table  0x{:05X}\n\n",
        a.ic_mod(),
        curves::KURAL_TABLOSU
    ));
    for (ad, e) in [("Fan 1", a), ("Fan 2", b)] {
        o.push_str(&format!(
            "{ad}  source 0x{:05X}  rule {}\n",
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
        "\nRule record layout: { b0, 00, b2, internal_mode, 00, FF, address_hi, address_lo }\n\
         b0 selects a table within the group: 00 = fan 0, 10 = fan 1.\n\
         b0 = 30/40 tables were never observed (all measurements were on battery).\n\
         The sensor represented by the t2 column was not measured; the graph uses t1 only.",
    );
    o
}

impl App {
    fn panel(&self) -> Panel {
        self.nav
            .active_data::<Panel>()
            .copied()
            .unwrap_or(Panel::Status)
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
                    .push(widget::text::title4("Missing capabilities"))
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
    /// varlık sebebi — the legacy driver's behavior tam olarak yazdığını
    /// bildirmekti.
    /// Klavye eylemini gönderir ve durumu HEMEN geri okur — `uygula()` ile
    /// aynı sözleşme: ekranda yazdığımız değil, sistemin döndürdüğü duruyor.
    fn kbd_uygula(&mut self, e: &klavye::Eylem) {
        match klavye::uygula(e) {
            Ok(()) => self.hata = None,
            Err(m) => self.hata = Some(m),
        }
        self.kbd = klavye::Durum::oku();
        if !self.kbd_surukleniyor {
            self.kbd_parlaklik_taslak = self.kbd.parlaklik;
        }
    }

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
                    .push(widget::button::text("Dismiss").on_press(Message::HataDismiss)),
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
            .title("Presets")
            .add(widget::text::caption(
                "Each preset applies a consistent fan mode and performance profile. \
                     Use the other panels for individual controls.",
            ));

        let eslesen = ON_AYARLAR.iter().position(|o| {
            s.fan_mode == Some(o.fan) && s.platform_profile.as_deref() == Some(o.profil)
        });

        // KARMA DURUMUN EKRANDA ADI OLMALI (12 Eyl 2026).
        //
        // Bu liste yalnız TAM eşleşmeyi biliyordu: fan modu ile profil aynı ön
        // ayara ait değilse hiçbir satır "Active" olmuyor ve arayüz "hiçbir şey
        // seçili değil" gibi okunuyordu. Oysa makinenin bir durumu var — sadece
        // adı yoktu. Karma duruma en az üç yoldan giriliyor:
        //   1. Oyun oturumu fanı `turbo`ya alır ama profili `balanced` bırakır
        //      (~/nixos-zixar/system/kernel/sched.nix). Oyun kapandıktan sonra
        //      geri dönüş kolu kırıksa bu hâl KALICI olur — kullanıcının 12 Eyl'de
        //      bildirdiği arıza tam olarak buydu.
        //   2. Fan panelinden tek bir mod seçmek (ön ayar paketi değil).
        //   3. Sürücü henüz profil yazmadıysa profil `custom` okunur; hiçbir ön
        //      ayarın `profil` alanı `custom` değil.
        // Çözüm gizlemek değil söylemek: ne olduğunu aynen yaz.
        if eslesen.is_none() {
            let fan = s.fan_mode.map_or_else(
                || {
                    s.fan_mode_raw_unknown
                        .clone()
                        .map_or_else(|| "—".to_string(), |r| format!("unknown ({r})"))
                },
                |m| m.label().to_string(),
            );
            let profil = s.platform_profile.clone().unwrap_or_else(|| "—".into());
            sec = sec.add(
                widget::settings::item::builder("Custom")
                    .description(format!(
                        "No preset matches the current state — fan: {fan}, profile: {profil}. \
                         Apply one below to bring both back in step."
                    ))
                    .control(widget::text::caption("active")),
            );
        }

        for (i, o) in ON_AYARLAR.iter().enumerate() {
            let dugme = if eslesen == Some(i) {
                widget::button::suggested("Active")
            } else {
                widget::button::standard("Apply").on_press(Message::OnAyarApply(i))
            };
            sec = sec.add(
                widget::settings::item::builder(o.ad)
                    .description(o.aciklama)
                    .control(dugme),
            );
        }

        let sarj = widget::settings::section().title("Charge limit").add(
            widget::settings::item::builder(format!("%{}", self.sarj_taslak))
                .description(
                    "80 is the configured default for battery longevity; use 100 before a long trip. \
                     A value set here is temporary — the declarative one in \
                     ~/nixos-zixar/system/arch/aerox16/wmi.nix wins on the next boot or switch.",
                )
                .control(
                    widget::slider(1..=100u8, self.sarj_taslak, Message::SarjKaydir)
                        .on_release(Message::SarjApply)
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
            .title("Live")
            .add(self.satir(
                "CPU temperature",
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
                "Power source",
                match s.ac_online {
                    Some(true) => "AC power".into(),
                    Some(false) => "Battery".into(),
                    None => Self::yok(),
                },
            ))
            .add(self.satir(
                "Fan mode",
                s.fan_mode.map_or_else(
                    || {
                        s.fan_mode_raw_unknown
                            .clone()
                            .map_or_else(Self::yok, |r| format!("unknown ({r})"))
                    },
                    |m| m.label().to_string(),
                ),
            ))
            .add(self.satir(
                "Performance profile",
                s.platform_profile.clone().unwrap_or_else(Self::yok),
            ))
            .add(self.satir(
                "Battery",
                s.battery_pct.map_or_else(Self::yok, |v| format!("%{v}")),
            ))
            .into()
    }

    fn fan_termal(&self) -> Element<'_, Message> {
        let s = &self.snap;

        let mut modlar = widget::settings::section().title("Fan mode");
        if s.fan_mode_choices.is_empty() {
            modlar = modlar.add(widget::text::body(
                "The driver did not provide a mode list — it may be missing or outdated.",
            ));
        } else {
            for m in &s.fan_mode_choices {
                let secili = s.fan_mode == Some(*m);
                let dugme = if secili {
                    widget::button::suggested("Active")
                } else {
                    widget::button::standard("Select").on_press(Message::FanModu(*m))
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
                    .title("Live")
                    .add(self.satir(
                        "CPU temperature",
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
                "Fan speed cannot be set directly on this firmware; only the active \
                 curve can be selected. (Measured: duty registers accept writes, \
                 but the fans do not respond.)",
            ))
            .push(widget::text::caption(
                "Open Fan curves to see a mode as a graph.",
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
            EgriSecim::Active => s.fan_mode,
            EgriSecim::Sabit(m) => Some(m),
        };
        // Active mod okunamıyorsa tek bir eğri çizmek uydurma olurdu; beş modu
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
            Some(m) => format!("Active mode — {}", m.label()),
            None => "Active mode — unavailable".into(),
        });
        secenekler.extend(EGRI_MODLARI.iter().map(|m| m.label().to_string()));

        let secili = match self.egri_secim {
            EgriSecim::Active => 0,
            EgriSecim::Sabit(m) => 1 + EGRI_MODLARI.iter().position(|x| *x == m).unwrap_or(0),
        };

        let mut gorunum = widget::settings::section()
            .title("View")
            .add(
                widget::settings::item::builder("Displayed curve")
                    .description("Changes the graph only; it does not change any machine setting.")
                    .control(widget::dropdown(secenekler, Some(secili), Message::EgriMod)),
            )
            .add(
                widget::settings::item::builder("Compare all five modes")
                    .description("Compare the effect of each mode at a glance.")
                    .control(
                        widget::toggler(self.egri_karsilastir).on_toggle(Message::EgriKarsilastir),
                    ),
            );

        if karsilastir {
            gorunum = gorunum.add(
                widget::settings::item::builder("Fan to compare")
                    .description("The comparison draws one fan; the two fan tables differ.")
                    .control(widget::dropdown(
                        vec!["Fan 1".to_string(), "Fan 2".to_string()],
                        Some(self.egri_fan as usize),
                        Message::EgriFan,
                    )),
            );
        }

        gorunum = gorunum.add(
            widget::settings::item::builder("Raw table and source")
                .description("14 rows × 3 columns, source offsets, and rule-table record.")
                .control(widget::toggler(self.egri_ham).on_toggle(Message::EgriHam)),
        );

        // --- canlı okuma: grafiğin DIŞINDA, sayı olarak (PLAN §4.1) ---
        let mut canli = widget::settings::section()
            .title("Live")
            .add(self.satir(
                "CPU temperature",
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
                "Expected duty (Fan 1 / Fan 2)",
                format!("{} / {}", d(0), d(1)),
            ));
        }

        let mut col = widget::column::with_capacity(12).spacing(24);

        if mod_bilinmiyor {
            col = col.push(widget::text::body(
                "The active fan mode could not be read, so all five modes are shown; \
                 choosing one would be guesswork.",
            ));
        }

        col = col.push(grafik);

        col = col.push(widget::text::caption(
            "This is a step plot, not an interpolated curve: thresholds were measured, \
             but behavior between thresholds was not. The graph exactly matches the \
             application calculation.",
        ));

        col = col.push(widget::text::caption(
            "The solid vertical line is the CPU temperature; the dashed line marks \
             the pointer. Which sensor the curve's `t1` column uses was not measured, \
             so the intersection duty is only an estimate.",
        ));

        col = col.push(gorunum).push(canli);

        if self.egri_ham
            && let Some(m) = gosterilen
        {
            col = col.push(
                widget::settings::section()
                    .title(format!("Raw table — {}", m.label()))
                    .add(widget::text::monotext(ham_tablo(m))),
            );
        }

        col.push(widget::text::caption(
            "Curves cannot be edited on this firmware; selecting a mode only chooses \
             which curve is loaded. (The data port is read-only; measured.)",
        ))
        .push(widget::text::caption(
            "There is no “Validate from EC” button: reading the live curve requires \
             the `EIDR` mailbox, but ACPI exposes no timeout flag (`ECTE`). Stale data \
             cannot be distinguished from a valid response, so this read is not offered \
             (`kernel/README.md`). The tables come from the firmware image, and all \
             eight measured active tables match their source.",
        ))
        .into()
    }

    fn guc_performans(&self) -> Element<'_, Message> {
        let s = &self.snap;
        let mut sec = widget::settings::section().title("Performance profile");

        sec = sec.add(self.satir(
            "Active",
            s.platform_profile.clone().unwrap_or_else(Self::yok),
        ));
        for p in &s.platform_profile_choices {
            let etkin = s.platform_profile.as_deref() == Some(p.as_str());
            let dugme = if etkin {
                widget::button::suggested("Active")
            } else {
                widget::button::standard("Select").on_press(Message::Profil(p.clone()))
            };
            sec = sec.add(widget::settings::item::builder(p.clone()).control(dugme));
        }

        let mut col = widget::column::with_capacity(8).spacing(24).push(sec);

        if s.platform_profile.as_deref() == Some("custom") {
            col = col.push(widget::text::caption(
                "“custom” means the driver has not written a profile yet. The EC does \
                 not expose a read for 0xED, so the active value is not guessed; it \
                 becomes known after the first profile change.",
            ));
        }

        col.push(widget::text::caption(
            "The profile switches CPU power limits and the dGPU boost budget as one \
             package. Fan mode is separate because the power manager rewrites the \
             profile when AC state changes.",
        ))
        .into()
    }

    /// Klavye aydınlatması paneli.
    ///
    /// Donanıma buradan DOĞRUDAN yazılmıyor: her kontrol `kbd-rgb`'yi
    /// çağırıyor (bkz. `klavye.rs` başlığı). Cihaz yoksa kontroller hiç
    /// çizilmiyor, yerine tek bir açıklama duruyor — düğmeye basıp hata almak
    /// yerine neden çalışmadığını okumak daha iyi.
    fn klavye_isik(&self) -> Element<'_, Message> {
        let k = &self.kbd;

        if !k.var() {
            return widget::column::with_capacity(2)
                .spacing(24)
                .push(
                    widget::settings::section()
                        .title("Keyboard light")
                        .add(self.satir("Device", Self::yok())),
                )
                .push(widget::text::caption(
                    "No LampArray device found. The internal keyboard exposes it as a HID \
                     interface; if it is missing, the kbd-rgb tool or its udev rule is not \
                     installed. Try `kbd-rgb info` in a terminal.",
                ))
                .into();
        }

        let durum = widget::settings::section()
            .title("Keyboard light")
            .add(self.satir("Colour", format!("#{}", k.renk)))
            .add(self.satir("Brightness", format!("%{}", k.parlaklik)))
            .add(self.satir(
                "State",
                if k.acik { "On".into() } else { "Off".into() },
            ))
            .add(self.satir(
                "Animation",
                k.animasyon.clone().unwrap_or_else(|| "None".into()),
            ))
            .add(
                widget::settings::item::builder("Power")
                    .description(
                        "Turning the light off keeps the colour and brightness, so turning it \
                         back on restores exactly what was there.",
                    )
                    .control(
                        widget::button::standard(if k.acik { "Turn off" } else { "Turn on" })
                            .on_press(Message::KbdToggle),
                    ),
            );

        let parlaklik = widget::settings::section().title("Brightness").add(
            widget::settings::item::builder(format!("%{}", self.kbd_parlaklik_taslak))
                .description(
                    "The hardware has no separate brightness channel, so this scales the RGB \
                     values. Raising it from zero also switches the light back on.",
                )
                .control(
                    widget::slider(
                        0..=100u32,
                        self.kbd_parlaklik_taslak,
                        Message::KbdParlaklikKaydir,
                    )
                    .on_release(Message::KbdParlaklikApply)
                    .width(Length::Fixed(240.0)),
                ),
        );

        // Hazır renkler iki satıra bölünüyor: dokuz düğme tek satırda dar
        // pencerede taşıyor.
        let mut ust = widget::row::with_capacity(5).spacing(8);
        let mut alt = widget::row::with_capacity(4).spacing(8);
        for (i, (ad, hex)) in klavye::ON_AYAR_RENKLER.iter().enumerate() {
            let dugme = if k.renk == *hex {
                widget::button::suggested(*ad)
            } else {
                widget::button::standard(*ad).on_press(Message::KbdRenk((*hex).to_string()))
            };
            if i < 5 {
                ust = ust.push(dugme);
            } else {
                alt = alt.push(dugme);
            }
        }

        let renk = widget::settings::section()
            .title("Colour")
            .add(widget::settings::item::builder("Presets").control(
                widget::column::with_capacity(2).spacing(8).push(ust).push(alt),
            ))
            .add(
                widget::settings::item::builder("Custom")
                    .description("Six hex digits, with or without '#'. Press Enter to apply.")
                    .control(
                        widget::text_input("8ba4b0", &self.kbd_hex)
                            .on_input(Message::KbdHexDuzenle)
                            .on_submit(|_| Message::KbdHexApply)
                            .width(Length::Fixed(140.0)),
                    ),
            )
            .add(
                widget::settings::item::builder("Theme colour")
                    .description(
                        "Restores the Stylix accent that the login service writes. The colour \
                         itself lives in lib/theme.nix — changing it there and rebuilding is \
                         what makes it stick.",
                    )
                    .control(
                        widget::button::standard("Back to theme").on_press(Message::KbdStylix),
                    ),
            );

        let mut anim_satir = widget::row::with_capacity(3).spacing(8);
        let kapali = k.animasyon.is_none();
        anim_satir = anim_satir.push(if kapali {
            widget::button::suggested("None")
        } else {
            widget::button::standard("None").on_press(Message::KbdAnimasyon(None))
        });
        for (ad, m) in klavye::ANIMASYONLAR {
            let etkin = k.animasyon.as_deref() == Some(*m);
            anim_satir = anim_satir.push(if etkin {
                widget::button::suggested(*ad)
            } else {
                widget::button::standard(*ad)
                    .on_press(Message::KbdAnimasyon(Some((*m).to_string())))
            });
        }

        let animasyon = widget::settings::section()
            .title("Animation")
            .add(
                widget::settings::item::builder("Effect")
                    .description(
                        "Animations run as a user service and are never started at login — \
                         nothing spins while idle. Stopping one hands the effects back to the \
                         firmware.",
                    )
                    .control(anim_satir),
            );

        widget::column::with_capacity(6)
            .spacing(24)
            .push(durum)
            .push(parlaklik)
            .push(renk)
            .push(animasyon)
            .push(widget::text::caption(
                "This panel drives the kbd-rgb tool, which talks to the keyboard over HID \
                 LampArray — a single zone in 8-bit colour. The keyboard also speaks a vendor \
                 protocol with per-key control and 13 hardware modes; that path is not wired \
                 up here yet.",
            ))
            .into()
    }

    fn pil(&self) -> Element<'_, Message> {
        let s = &self.snap;
        widget::column::with_capacity(8)
            .spacing(24)
            .push(
                widget::settings::section()
                    .title("Battery")
                    .add(self.satir(
                        "Charge",
                        s.battery_pct.map_or_else(Self::yok, |v| format!("%{v}")),
                    ))
                    .add(
                        self.satir(
                            "Charge limit",
                            s.charge_limit_pct
                                .map_or_else(Self::yok, |v| format!("%{v}")),
                        ),
                    )
                    .add(self.satir(
                        "Power source",
                        match s.ac_online {
                            Some(true) => "AC power".into(),
                            Some(false) => "Battery".into(),
                            None => Self::yok(),
                        },
                    )),
            )
            .push(widget::text::caption(
                "The charge limit uses the standard kernel interface. Each write is \
                 read back and verified, and the value is restored after resume.",
            ))
            .into()
    }

    fn hakkinda(&self) -> Element<'_, Message> {
        widget::column::with_capacity(8)
            .spacing(16)
            .push(widget::text::title3("AERO Control"))
            .push(widget::text::body(
                "Control interface for the Gigabyte AERO X16 1VH (SKU EG61VH).",
            ))
            .push(widget::text::caption(
                "Only measured capabilities are exposed. Fan speed is not directly \
                 adjustable, the socket-temperature channel is inactive, and the \
                 keyboard-light register has no visible effect; none is presented \
                 as a control.",
            ))
            .push(widget::text::caption(format!(
                "Driver: {}",
                if self.snap.driver_present {
                    "aero_eg61h loaded"
                } else {
                    "NOT LOADED"
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
            .text("Presets")
            .icon(widget::icon::from_name("preferences-system-symbolic"))
            .data(Panel::OnAyarlar)
            .activate();
        nav.insert()
            .text("Status")
            .icon(widget::icon::from_name("utilities-system-monitor-symbolic"))
            .data(Panel::Status);
        nav.insert()
            .text("Fan and thermal")
            .icon(gomulu_simge(include_bytes!(
                "../data/icons/fan-symbolic.svg"
            )))
            .data(Panel::FanTermal);
        nav.insert()
            .text("Fan curves")
            .icon(gomulu_simge(include_bytes!(
                "../data/icons/fan-curve-symbolic.svg"
            )))
            .data(Panel::FanEgrisi);
        nav.insert()
            .text("Power and performance")
            .icon(gomulu_simge(include_bytes!(
                "../data/icons/power-symbolic.svg"
            )))
            .data(Panel::GucPerformance);
        nav.insert()
            .text("Battery")
            .icon(widget::icon::from_name("battery-symbolic"))
            .data(Panel::Battery);
        nav.insert()
            .text("Keyboard light")
            .icon(widget::icon::from_name("input-keyboard-symbolic"))
            .data(Panel::KlavyeIsik);
        nav.insert()
            .text("About")
            .icon(widget::icon::from_name("help-about-symbolic"))
            .data(Panel::Hakkinda);

        let snap = Snapshot::read();
        let kbd = klavye::Durum::oku();
        let app = App {
            core,
            nav,
            sarj_taslak: snap.charge_limit_pct.unwrap_or(80),
            sarj_surukleniyor: false,
            snap,
            hata: None,
            egri_secim: EgriSecim::Active,
            egri_karsilastir: false,
            egri_fan: 0,
            egri_ham: false,
            kbd_parlaklik_taslak: kbd.parlaklik,
            kbd_surukleniyor: false,
            kbd_hex: kbd.renk.clone(),
            kbd,
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
                if !self.sarj_surukleniyor {
                    if let Some(v) = self.snap.charge_limit_pct {
                        self.sarj_taslak = v;
                    }
                }
                // Klavye durumu YALNIZ o panel görünürken tazeleniyor: okuma
                // sysfs değil, iki alt süreç (kbd-rgb + systemctl). Arka planda
                // her tikte süreç açmanın karşılığı yok.
                if self.panel() == Panel::KlavyeIsik {
                    self.kbd = klavye::Durum::oku();
                    if !self.kbd_surukleniyor {
                        self.kbd_parlaklik_taslak = self.kbd.parlaklik;
                    }
                }
            }

            Message::HataDismiss => self.hata = None,

            // Eğri paneli: dördü de yalnız ÇİZİMİ değiştiriyor, donanıma
            // hiçbir yazma yapmıyor.
            Message::EgriMod(i) => {
                self.egri_secim = match i.checked_sub(1).and_then(|k| EGRI_MODLARI.get(k)) {
                    Some(m) => EgriSecim::Sabit(*m),
                    // 0 ya da beklenmedik indeks: etkin modu izlemeye dön.
                    None => EgriSecim::Active,
                };
            }

            Message::EgriKarsilastir(v) => self.egri_karsilastir = v,

            // Sürücü iki fan sunuyor; başka bir indeks gelirse fan 1'e düş.
            Message::EgriFan(i) => self.egri_fan = u8::from(i == 1),

            Message::EgriHam(v) => self.egri_ham = v,

            Message::SarjKaydir(v) => {
                self.sarj_surukleniyor = true;
                self.sarj_taslak = v;
            }

            Message::SarjApply => {
                self.sarj_surukleniyor = false;
                self.uygula(Action::ChargeLimit(self.sarj_taslak));
            }

            Message::FanModu(m) => self.uygula(Action::FanMode(m)),

            Message::Profil(p) => self.uygula(Action::Profile(p)),

            // --- Klavye aydınlatması ------------------------------------
            Message::KbdParlaklikKaydir(v) => {
                self.kbd_surukleniyor = true;
                self.kbd_parlaklik_taslak = v;
            }

            Message::KbdParlaklikApply => {
                self.kbd_surukleniyor = false;
                self.kbd_uygula(&klavye::Eylem::Parlaklik(self.kbd_parlaklik_taslak));
            }

            Message::KbdRenk(hex) => {
                self.kbd_hex = hex.clone();
                self.kbd_uygula(&klavye::Eylem::Renk(hex));
            }

            Message::KbdHexDuzenle(s) => self.kbd_hex = s,

            // Doğrulama YAZMADAN önce: geçersiz giriş alt sürece hiç gitmiyor,
            // kullanıcı da neyin yanlış olduğunu hata şeridinde görüyor.
            Message::KbdHexApply => match klavye::hex_temizle(&self.kbd_hex) {
                Some(h) => {
                    self.kbd_hex = h.clone();
                    self.kbd_uygula(&klavye::Eylem::Renk(h));
                }
                None => {
                    self.hata = Some(format!(
                        "'{}' altı haneli bir hex renk değil (örnek: 8ba4b0)",
                        self.kbd_hex
                    ));
                }
            },

            Message::KbdToggle => self.kbd_uygula(&klavye::Eylem::Toggle),

            Message::KbdAnimasyon(m) => self.kbd_uygula(&klavye::Eylem::Animasyon(m)),

            Message::KbdStylix => self.kbd_uygula(&klavye::Eylem::StylixDon),

            Message::OnAyarApply(i) => {
                if let Some(o) = ON_AYARLAR.get(i) {
                    // Fan mode ÖNCE: profil yazımı PPD üzerinden gidiyor ve
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
            // Eğri panelinde canlı imleç var — Status/Fan ile aynı hızda.
            Panel::Status | Panel::FanTermal | Panel::FanEgrisi => Duration::from_secs(2),
            Panel::OnAyarlar => Duration::from_secs(3),
            _ => Duration::from_secs(5),
        };
        time::every(period).map(|_| Message::Tik)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icerik = match self.panel() {
            Panel::OnAyarlar => self.on_ayarlar(),
            Panel::Status => self.durum(),
            Panel::FanTermal => self.fan_termal(),
            Panel::FanEgrisi => self.fan_egrisi(),
            Panel::GucPerformance => self.guc_performans(),
            Panel::Battery => self.pil(),
            Panel::KlavyeIsik => self.klavye_isik(),
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
        // İlk satır Balanced'nin ölçülen ilk noktası: fan 0 54/70/18, fan 1 48/57/18.
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
