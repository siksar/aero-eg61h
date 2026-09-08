// SPDX-License-Identifier: GPL-2.0-only
//! AERO Kontrol — Gigabyte AERO X16 1VH (EG61VH) kontrol arayüzü.
//!
//! # Bu sürüm SALT OKUNUR
//!
//! Yazma yolu henüz yok: yetki köprüsü kararı verilmedi (`PLAN.md` §10 adım 6).
//! Okuduğu dört değerin dördü de dünyaya-okunur, yani bu uygulama **root
//! olmadan** çalışıyor ve hiçbir yetki istemiyor.
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

use aero_sysfs::Snapshot;
use cosmic::app::{Core, Settings, Task};
use cosmic::iced::{Alignment, Length, Subscription, time};
use cosmic::widget::{self, segmented_button};
use cosmic::{Apply, Application, Element, executor};
use std::time::Duration;

const APP_ID: &str = "dev.zixar.AeroControl";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Panel {
    Durum,
    FanTermal,
    GucPerformans,
    Pil,
    Hakkinda,
}

#[derive(Debug, Clone)]
enum Message {
    /// Zamanlayıcı tetikledi — sysfs'i yeniden oku.
    Tik,
}

struct App {
    core: Core,
    nav: segmented_button::SingleSelectModel,
    snap: Snapshot,
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
            col = col.push(
                widget::text::body(format!("{} — {}", p.what, p.why))
                    .width(Length::Fill),
            );
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
                modlar = modlar.add(
                    widget::settings::item::builder(m.label())
                        .description(m.describe())
                        .control(widget::text::body(if secili { "● etkin" } else { "" })),
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
                "Bu sürüm salt okunur — mod değiştirme yetki köprüsü geldiğinde açılacak.",
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
        if !s.platform_profile_choices.is_empty() {
            sec = sec.add(self.satir(
                "Seçenekler",
                s.platform_profile_choices.join(", "),
            ));
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
                    .add(self.satir(
                        "Şarj limiti",
                        s.charge_limit_pct.map_or_else(Self::yok, |v| format!("%{v}")),
                    ))
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
            .text("Durum")
            .icon(widget::icon::from_name("utilities-system-monitor-symbolic"))
            .data(Panel::Durum)
            .activate();
        nav.insert()
            .text("Fan ve Termal")
            .icon(widget::icon::from_name("temperature-symbolic"))
            .data(Panel::FanTermal);
        nav.insert()
            .text("Güç ve Performans")
            .icon(widget::icon::from_name("power-profile-performance-symbolic"))
            .data(Panel::GucPerformans);
        nav.insert()
            .text("Pil")
            .icon(widget::icon::from_name("battery-symbolic"))
            .data(Panel::Pil);
        nav.insert()
            .text("Hakkında")
            .icon(widget::icon::from_name("help-about-symbolic"))
            .data(Panel::Hakkinda);

        let app = App {
            core,
            nav,
            snap: Snapshot::read(),
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
            Message::Tik => self.snap = Snapshot::read(),
        }
        Task::none()
    }

    /// Örnekleme YALNIZ uygulama açıkken. Görünür panele göre yavaşlıyor:
    /// canlı sayı göstermeyen panellerde EC'yi boşuna yormanın anlamı yok.
    fn subscription(&self) -> Subscription<Self::Message> {
        let period = match self.panel() {
            Panel::Durum | Panel::FanTermal => Duration::from_secs(2),
            _ => Duration::from_secs(5),
        };
        time::every(period).map(|_| Message::Tik)
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let icerik = match self.panel() {
            Panel::Durum => self.durum(),
            Panel::FanTermal => self.fan_termal(),
            Panel::GucPerformans => self.guc_performans(),
            Panel::Pil => self.pil(),
            Panel::Hakkinda => self.hakkinda(),
        };

        let mut col = widget::column::with_capacity(8).spacing(24);
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
    cosmic::app::run::<App>(Settings::default().size_limits(
        cosmic::iced::Limits::NONE.min_width(420.0).min_height(320.0),
    ), ())
}
