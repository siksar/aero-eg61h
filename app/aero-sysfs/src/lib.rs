// SPDX-License-Identifier: GPL-2.0-only
//! `aero_eg61h` sürücüsünün sysfs yüzeyini okur.
//!
//! # Tasarımın tek kuralı: HER DÜĞÜM OPSİYONEL
//!
//! The driver may be absent or only partially installed. This layer never
//! panics when an optional interface is unavailable: unreadable values become
//! `None` and the reason is reported through [`Snapshot::problems`]. The GUI
//! shows that list in its status area.
//!
//! Bu, iki iş kolunun buluştuğu sözleşme: GUI, NixOS geçişini beklemeden
//! geliştirilebilir ve geçiş yapılmamış bir makinede de anlamlı bir şey gösterir.
//!
//! # Yetki
//!
//! Buradaki her şey **salt okunur** ve dört değerin dördü de dünyaya-okunur
//! (ölçüldü, 7 Eyl 2026), yani bu katman **hiç yetki istemiyor**. Yazma yolu
//! ayrı bir katman olacak (yetki köprüsü kararı henüz verilmedi).

pub mod curves;
pub mod write;
pub use write::{Action, Error as WriteError, apply};

use std::fs;
use std::path::{Path, PathBuf};

/// `WMBD` GUID'i — fan mode düğümü bu cihazın altında.
/// Sondaki örnek indeksi (`-2`) `_WDG` sırasından geliyor; sabit varsaymıyoruz,
/// önekle arıyoruz.
const WMBD_GUID_PREFIX: &str = "ABBC0F75-8EA1-11D1-00A0-C90629100000";

const HWMON_NAME: &str = "aero_eg61h";
const BATTERY: &str = "BAT1";
const AC_ADAPTER: &str = "ACAD";

/// Fan mode. İsimler sürücünün `fan_mode_choices` çıktısıyla birebir.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FanMode {
    /// 54 °C'de başlar, tavan %29
    Quiet,
    /// mod 4 — 54 °C'de başlar, tavan %43. Makinenin varsayılanı.
    Balanced,
    /// mod 0 — 40 °C'de başlar, tavan %43
    Responsive,
    /// 40 °C'de başlar, tavan %53
    Gaming,
    /// 36 °C'de başlar, düz %63
    Turbo,
}

impl FanMode {
    pub fn from_sysfs(s: &str) -> Option<Self> {
        match s.trim() {
            "quiet" => Some(Self::Quiet),
            "balanced" => Some(Self::Balanced),
            "responsive" => Some(Self::Responsive),
            "gaming" => Some(Self::Gaming),
            "turbo" => Some(Self::Turbo),
            _ => None,
        }
    }

    pub fn as_sysfs(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Balanced => "balanced",
            Self::Responsive => "responsive",
            Self::Gaming => "gaming",
            Self::Turbo => "turbo",
        }
    }

    /// Kullanıcıya gösterilecek ad.
    pub fn label(self) -> &'static str {
        match self {
            Self::Quiet => "Quiet",
            Self::Balanced => "Balanced",
            Self::Responsive => "Responsive",
            Self::Gaming => "Performance",
            Self::Turbo => "Turbo",
        }
    }

    /// Eğrinin tek cümlelik özeti. Firmware tablolarından (ölçüldü).
    pub fn describe(self) -> &'static str {
        match self {
            Self::Quiet => "starts at 54 °C, maximum 29%",
            Self::Balanced => "starts at 54 °C, maximum 43%",
            Self::Responsive => "starts at 40 °C, maximum 43%",
            Self::Gaming => "starts at 40 °C, maximum 53%",
            Self::Turbo => "starts at 36 °C, fixed 63%",
        }
    }
}

/// Okunamayan bir şeyin insanca açıklaması. GUI bunları üst şeritte gösterir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// Hangi yetenek eksik.
    pub what: &'static str,
    /// Neden — kullanıcının yapabileceği bir şey varsa onu söyler.
    pub why: String,
}

/// Tek bir okuma anının tamamı.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    /// Sürücünün hwmon cihazı bulundu mu — "sürücü yüklü mü"nün pratik cevabı.
    pub driver_present: bool,

    pub cpu_temp_c: Option<u8>,
    pub fan1_rpm: Option<u32>,
    pub fan2_rpm: Option<u32>,

    pub fan_mode: Option<FanMode>,
    /// Sürücünün sunduğu modlar. Sabit varsaymıyoruz — sürücüden okuyoruz.
    pub fan_mode_choices: Vec<FanMode>,
    /// `fan_mode` düğümü tanınmayan bir değer okudu (olmaması gereken durum).
    pub fan_mode_raw_unknown: Option<String>,

    pub charge_limit_pct: Option<u8>,
    pub battery_pct: Option<u8>,

    /// Bizim handler'ımızın profili. `custom` = sürücü henüz yazmadı, EC'nin
    /// durumu bilinmiyor (WMBC'de 0xED okuması yok).
    pub platform_profile: Option<String>,
    pub platform_profile_choices: Vec<String>,

    pub ac_online: Option<bool>,

    pub problems: Vec<Problem>,
}

fn read_trim(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

fn read_num<T: std::str::FromStr>(p: impl AsRef<Path>) -> Option<T> {
    read_trim(p)?.parse().ok()
}

/// `/sys/class/<class>` altında `name` dosyası `want` olan ilk dizini bulur.
fn find_by_name(root: &Path, class: &str, want: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root.join("sys/class").join(class)).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        if read_trim(p.join("name")).as_deref() == Some(want) {
            return Some(p);
        }
    }
    None
}

/// `/sys/bus/wmi/devices/<GUID>-<n>` — örnek indeksi bilinmiyor, önekle ara.
fn find_wmi_device(root: &Path, guid_prefix: &str) -> Option<PathBuf> {
    let entries = fs::read_dir(root.join("sys/bus/wmi/devices")).ok()?;
    for e in entries.flatten() {
        if e.file_name().to_string_lossy().starts_with(guid_prefix) {
            return Some(e.path());
        }
    }
    None
}

impl Snapshot {
    /// Her şeyi bir kez okur. Yoklama yapmaz, arka planda hiçbir şey bırakmaz —
    /// çağıran ne sıklıkta çağıracağına kendi karar verir (GUI: yalnız açıkken).
    pub fn read() -> Self {
        Self::read_from(Path::new("/"))
    }

    /// Kökü verilerek okur. Üretimde kök `/`; testlerde sahte bir sysfs ağacı.
    /// Bu, "sürücü yok" yolunun makineye dokunmadan sınanmasını sağlıyor —
    /// o yol iki iş kolunun sözleşmesi olduğu için sınanmadan bırakılamaz.
    pub fn read_from(root: &Path) -> Self {
        let mut s = Snapshot::default();

        s.read_hwmon(root);
        s.read_fan_mode(root);
        s.read_battery(root);
        s.read_platform_profile(root);
        s.read_ac(root);

        s
    }

    fn miss(&mut self, what: &'static str, why: impl Into<String>) {
        self.problems.push(Problem { what, why: why.into() });
    }

    fn read_hwmon(&mut self, root: &Path) {
        let Some(dir) = find_by_name(root, "hwmon", HWMON_NAME) else {
            self.miss(
                "Temperature and fan speed",
                format!(
                    "`{HWMON_NAME}` hwmon device is missing — the driver is not loaded. \
                     Load it with: sudo insmod ~/aero-eg61h/kernel/aero-eg61h.ko"
                ),
            );
            return;
        };

        self.driver_present = true;
        // millidereceden dereceye. Okunamıyorsa sessizce None — sürücü var ama
        // EC cevap vermiyorsa bu geçici bir durum olabilir, gürültü yapma.
        self.cpu_temp_c = read_num::<i32>(dir.join("temp1_input")).map(|m| (m / 1000) as u8);
        self.fan1_rpm = read_num(dir.join("fan1_input"));
        self.fan2_rpm = read_num(dir.join("fan2_input"));
    }

    fn read_fan_mode(&mut self, root: &Path) {
        let Some(dev) = find_wmi_device(root, WMBD_GUID_PREFIX) else {
            self.miss(
                "Fan mode",
                "The WMBD device (ABBC0F75-…) is missing from sysfs — the driver is not bound to the WMI bus.",
            );
            return;
        };

        let node = dev.join("fan_mode");
        match read_trim(&node) {
            None => self.miss(
                "Fan mode",
                "The WMBD device exists but has no `fan_mode` node — the driver may be outdated.",
            ),
            Some(raw) => match FanMode::from_sysfs(&raw) {
                Some(m) => self.fan_mode = Some(m),
                None => {
                    // Sürücü `unknown` yazdıysa EC tanınmayan bir desende.
                    // Uydurmuyoruz — ham değeri taşıyoruz.
                    self.fan_mode_raw_unknown = Some(raw.clone());
                    self.miss(
                        "Fan mode",
                        format!("The EC returned an unknown pattern (`{raw}`) — unexpected state."),
                    );
                }
            },
        }

        if let Some(list) = read_trim(dev.join("fan_mode_choices")) {
            self.fan_mode_choices = list.split_whitespace().filter_map(FanMode::from_sysfs).collect();
        }
    }

    fn read_battery(&mut self, root: &Path) {
        let bat = root.join("sys/class/power_supply").join(BATTERY);
        self.battery_pct = read_num(bat.join("capacity"));

        match read_num::<u8>(bat.join("charge_control_end_threshold")) {
            Some(v) => self.charge_limit_pct = Some(v),
            None => self.miss(
                "Charge limit",
                format!(
                    "`{BATTERY}/charge_control_end_threshold` is missing — the driver's \
                     power_supply extension is not available."
                ),
            ),
        }
    }

    fn read_platform_profile(&mut self, root: &Path) {
        let Some(dir) = find_by_name(root, "platform-profile", HWMON_NAME) else {
            self.miss(
                "Performance profile",
                "The driver's platform_profile handler is not registered.",
            );
            return;
        };

        self.platform_profile = read_trim(dir.join("profile"));
        if let Some(list) = read_trim(dir.join("choices")) {
            self.platform_profile_choices = list.split_whitespace().map(str::to_string).collect();
        }
    }

    fn read_ac(&mut self, root: &Path) {
        self.ac_online = read_num::<u8>(root.join("sys/class/power_supply").join(AC_ADAPTER).join("online"))
            .map(|v| v == 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn write(p: PathBuf, v: &str) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, v).unwrap();
    }

    /// Testler için sahte bir sysfs ağacı. `std::env::temp_dir()` altında,
    /// çağıran isme göre ayrılır (harici crate getirmemek için tempfile yok).
    fn fake_root(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("aero-sysfs-test-{name}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// Tam donanımlı sahte makine.
    fn saglikli(name: &str) -> PathBuf {
        let r = fake_root(name);
        let hw = r.join("sys/class/hwmon/hwmon9");
        write(hw.join("name"), "aero_eg61h\n");
        write(hw.join("temp1_input"), "51000\n");
        write(hw.join("fan1_input"), "0\n");
        write(hw.join("fan2_input"), "2345\n");

        let wmi = r.join("sys/bus/wmi/devices/ABBC0F75-8EA1-11D1-00A0-C90629100000-2");
        write(wmi.join("fan_mode"), "balanced\n");
        write(wmi.join("fan_mode_choices"), "quiet balanced responsive gaming turbo\n");

        let bat = r.join("sys/class/power_supply/BAT1");
        write(bat.join("capacity"), "58\n");
        write(bat.join("charge_control_end_threshold"), "60\n");
        write(r.join("sys/class/power_supply/ACAD/online"), "1\n");

        let pp = r.join("sys/class/platform-profile/platform-profile-1");
        write(pp.join("name"), "aero_eg61h\n");
        write(pp.join("profile"), "balanced\n");
        write(pp.join("choices"), "low-power balanced performance\n");
        r
    }

    #[test]
    fn fan_mode_gidis_donus() {
        for m in [
            FanMode::Quiet,
            FanMode::Balanced,
            FanMode::Responsive,
            FanMode::Gaming,
            FanMode::Turbo,
        ] {
            assert_eq!(FanMode::from_sysfs(m.as_sysfs()), Some(m));
        }
    }

    #[test]
    fn taninmayan_fan_modu_none() {
        // Sürücü tanınmayan desende `unknown` yazıyor; uydurma bir moda
        // eşlemek the legacy driver's behavior olurdu.
        assert_eq!(FanMode::from_sysfs("unknown"), None);
        assert_eq!(FanMode::from_sysfs("5"), None);
        assert_eq!(FanMode::from_sysfs(""), None);
    }

    #[test]
    fn saglikli_makine_hic_eksik_bildirmez() {
        let s = Snapshot::read_from(&saglikli("saglikli"));
        assert!(s.driver_present);
        assert_eq!(s.problems, vec![], "eksik olmamalı");
        assert_eq!(s.cpu_temp_c, Some(51));
        assert_eq!(s.fan1_rpm, Some(0), "0 rpm GEÇERLİ: mod 4 boşta fanı durduruyor");
        assert_eq!(s.fan2_rpm, Some(2345));
        assert_eq!(s.fan_mode, Some(FanMode::Balanced));
        assert_eq!(s.fan_mode_choices.len(), 5);
        assert_eq!(s.charge_limit_pct, Some(60));
        assert_eq!(s.battery_pct, Some(58));
        assert_eq!(s.ac_online, Some(true));
        assert_eq!(s.platform_profile.as_deref(), Some("balanced"));
    }

    /// İKİ İŞ KOLUNUN SÖZLEŞMESİ: sürücü yokken çökme, sebebini söyle.
    #[test]
    fn surucu_yokken_cokmez_ve_sebep_yazar() {
        let r = fake_root("bos"); // hiçbir şey yok
        let s = Snapshot::read_from(&r);

        assert!(!s.driver_present);
        assert_eq!(s.cpu_temp_c, None);
        assert_eq!(s.fan_mode, None);
        assert_eq!(s.charge_limit_pct, None);

        // Dört yeteneğin dördü de sebebiyle bildirilmeli.
        let neler: Vec<_> = s.problems.iter().map(|p| p.what).collect();
        assert!(neler.contains(&"Temperature and fan speed"), "{neler:?}");
        assert!(neler.contains(&"Fan mode"), "{neler:?}");
        assert!(neler.contains(&"Charge limit"), "{neler:?}");
        assert!(neler.contains(&"Performance profile"), "{neler:?}");

        // Sebepler boş olmamalı — kullanıcı ne yapacağını bilmeli.
        for p in &s.problems {
            assert!(!p.why.is_empty(), "{} için sebep yok", p.what);
        }
    }

    /// Sürücü var ama EC tanınmayan bir desende: uydurma, ham değeri taşı.
    #[test]
    fn taninmayan_desen_uydurulmaz() {
        let r = saglikli("unknown-desen");
        write(
            r.join("sys/bus/wmi/devices/ABBC0F75-8EA1-11D1-00A0-C90629100000-2/fan_mode"),
            "unknown\n",
        );
        let s = Snapshot::read_from(&r);

        assert!(s.driver_present);
        assert_eq!(s.fan_mode, None, "tanınmayan desen bir moda EŞLENMEMELİ");
        assert_eq!(s.fan_mode_raw_unknown.as_deref(), Some("unknown"));
        assert!(s.problems.iter().any(|p| p.what == "Fan mode"));
    }

    /// Sürücü yüklü ama platform_profile katmanı kurulmamış (kısmi durum).
    #[test]
    fn kismi_kurulum_yalniz_eksigi_bildirir() {
        let r = saglikli("kismi");
        fs::remove_dir_all(r.join("sys/class/platform-profile")).unwrap();
        let s = Snapshot::read_from(&r);

        assert!(s.driver_present, "hwmon hâlâ var");
        assert_eq!(s.cpu_temp_c, Some(51), "diğer okumalar etkilenmemeli");
        assert_eq!(s.problems.len(), 1);
        assert_eq!(s.problems[0].what, "Performance profile");
    }

    /// `custom` geçerli bir değer — sürücü henüz yazmadı demek, hata değil.
    #[test]
    fn custom_profil_hata_degil() {
        let r = saglikli("custom");
        write(
            r.join("sys/class/platform-profile/platform-profile-1/profile"),
            "custom\n",
        );
        let s = Snapshot::read_from(&r);
        assert_eq!(s.platform_profile.as_deref(), Some("custom"));
        assert_eq!(s.problems, vec![]);
    }

    #[test]
    fn gercek_makinede_panik_etmez() {
        let s = Snapshot::read();
        if !s.driver_present {
            assert!(!s.problems.is_empty(), "sürücü yokken sebep yazılmalı");
        }
    }
}
