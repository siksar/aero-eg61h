// SPDX-License-Identifier: GPL-2.0-only
//! Yazma köprüsü.
//!
//! # Neden burada doğrudan sysfs'e yazmıyoruz
//!
//! Düğümler root'a ait. GUI root koşmayacak. Ama bunun için bir D-Bus daemon
//! GEREKMİYOR — 8 Eyl 2026'da ölçüldü ki yetkisiz kullanıcı zaten iki köprüyü
//! kullanabiliyor:
//!
//! ```text
//! powerprofilesctl set power-saver  ->  platform_profile = low-power  ✓
//! systemctl start aero-set-fan@quiet  ->  fan mode değişti            ✓
//! ```
//!
//! Yani bu katman **var olan brokerleri çağırıyor**, yeni bir tane kurmuyor:
//!
//! | eylem | köprü |
//! |---|---|
//! | Performance profile | `powerprofilesctl` (PPD, D-Bus + polkit) |
//! | Fan mode | `systemctl start aero-set-fan@<mod>` (polkit kuralı) |
//! | Charge limit | `systemctl start aero-set-charge@<yüzde>` (polkit kuralı) |
//!
//! # Doğrulama nerede
//!
//! Buradaki aralık denetimi yalnız **hızlı geri bildirim** için. Asıl otorite
//! systemd biriminin içindeki beyaz liste — `%i` kullanıcıdan geldiği için
//! doğrulamanın ayrıcalıklı tarafta olması şart. Buradaki kontrolü kaldırmak
//! sistemi güvensiz yapmaz, yalnız hata mesajını geç ve çirkin yapar.

use crate::FanMode;
use std::fmt;
use std::process::Command;

/// Nix paketlemesi mutlak yolları derleme anında gömebilsin diye.
/// Gömülmezse PATH'ten çözülür (geliştirme kolaylığı).
const SYSTEMCTL: &str = match option_env!("AERO_SYSTEMCTL") {
    Some(p) => p,
    None => "systemctl",
};
const POWERPROFILESCTL: &str = match option_env!("AERO_POWERPROFILESCTL") {
    Some(p) => p,
    None => "powerprofilesctl",
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    FanMode(FanMode),
    /// Yüzde, 1-100. 0 REDDEDİLİR: anlamı ölçülmedi ("hiç şarj etme" olabilir).
    ChargeLimit(u8),
    /// `platform_profile` adı — `Snapshot::platform_profile_choices`'tan gelmeli.
    Profile(String),
}

#[derive(Debug, Clone)]
pub enum Error {
    /// Girdi bu tarafta zaten geçersiz — köprüye hiç gitmedi.
    Gecersiz(String),
    /// Köprü could not run (binary yok, PATH boş…).
    Calistirilamadi { komut: String, sebep: String },
    /// Köprü çalıştı ama rejected. `stderr` kullanıcıya gösterilebilir.
    Reddedildi { komut: String, stderr: String },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gecersiz(m) => write!(f, "{m}"),
            Self::Calistirilamadi { komut, sebep } => {
                write!(f, "`{komut}` could not run: {sebep}")
            }
            Self::Reddedildi { komut, stderr } => {
                if stderr.trim().is_empty() {
                    write!(f, "`{komut}` rejected")
                } else {
                    write!(f, "{}", stderr.trim())
                }
            }
        }
    }
}

impl std::error::Error for Error {}

/// Kernel `platform_profile` ABI adını PPD'nin adına çevirir.
///
/// ÖLÇÜLDÜ 12 Eyl 2026 — bu fonksiyon olmadan profil yazma yolu YARIM ÇALIŞIYORDU.
/// Okuma tarafı sürücünün `platform-profile/*/choices` düğümünü okuyor ve orası
/// kernel ABI adlarını veriyor:
///
/// ```text
/// $ cat /sys/class/platform-profile/platform-profile-0/choices
/// low-power balanced performance
/// $ powerprofilesctl set low-power
/// error: argument profile: invalid choice: 'low-power'
///        (choose from 'power-saver', 'balanced', 'performance')
/// ```
///
/// Yani "Quiet" ön ayarı ve Güç panelindeki `low-power` düğmesi sessizce
/// reddediliyordu: fan modu yazılıyor, profil yazılmıyor, makine karma bir
/// durumda kalıyordu (hiçbir ön ayar "Active" görünmüyor).
///
/// ÇEVİRİ, KÖPRÜ DEĞİŞİKLİĞİ DEĞİL — bilerek. PPD tek profil otoritesi olarak
/// kalmalı: `~/nixos-zixar`'ın power-display.service'i ve sched.nix'i de profili
/// PPD üzerinden sürüyor. Handler düğümüne doğrudan yazmak PPD'nin iç durumunu
/// bozar ve bir sonraki AC/pil olayında geri alınır.
fn ppd_adi(kernel_adi: &str) -> &str {
    match kernel_adi.trim() {
        "low-power" => "power-saver",
        other => other,
    }
}

fn calistir(prog: &str, args: &[&str]) -> Result<(), Error> {
    let komut = format!("{prog} {}", args.join(" "));

    let out = Command::new(prog)
        .args(args)
        .output()
        .map_err(|e| Error::Calistirilamadi {
            komut: komut.clone(),
            sebep: e.to_string(),
        })?;

    if out.status.success() {
        return Ok(());
    }

    Err(Error::Reddedildi {
        komut,
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// Eylemi uygular. Başarı, köprünün 0 dönmesi demek — fan biriminin kendisi
/// yazımı geri okuyup doğruluyor, yani "0 döndü" gerçekten "tuttu" demek.
pub fn apply(action: &Action) -> Result<(), Error> {
    match action {
        Action::FanMode(m) => calistir(
            SYSTEMCTL,
            &["start", &format!("aero-set-fan@{}.service", m.as_sysfs())],
        ),

        Action::ChargeLimit(pct) => {
            if !(1..=100).contains(pct) {
                return Err(Error::Gecersiz(format!(
                    "charge limit must be between 1 and 100, got: {pct}"
                )));
            }
            calistir(
                SYSTEMCTL,
                &["start", &format!("aero-set-charge@{pct}.service")],
            )
        }

        Action::Profile(p) => {
            // Profil adları sürücüden okunuyor; burada uydurma bir liste
            // tutmuyoruz. Boş dize tek gerçek hata.
            if p.trim().is_empty() {
                return Err(Error::Gecersiz("profile name is empty".into()));
            }
            calistir(POWERPROFILESCTL, &["set", ppd_adi(p)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sarj_limiti_araligi_bu_tarafta_da_denetleniyor() {
        // Asıl otorite systemd birimi; bu yalnız hızlı geri bildirim.
        for kotu in [0u8, 101, 255] {
            let e = apply(&Action::ChargeLimit(kotu)).unwrap_err();
            assert!(matches!(e, Error::Gecersiz(_)), "{kotu} kabul edildi: {e:?}");
        }
    }

    #[test]
    fn bos_profil_reddediliyor() {
        let e = apply(&Action::Profile("  ".into())).unwrap_err();
        assert!(matches!(e, Error::Gecersiz(_)));
    }

    /// Kernel ABI adı ile PPD'nin adı yalnız bir noktada ayrışıyor; ayrıştığı
    /// yer de çevrilmezse "Quiet" ön ayarı yarım uygulanıyor (12 Eyl 2026).
    #[test]
    fn kernel_profil_adi_ppd_adina_cevriliyor() {
        assert_eq!(ppd_adi("low-power"), "power-saver");
        assert_eq!(ppd_adi("balanced"), "balanced");
        assert_eq!(ppd_adi("performance"), "performance");
        // Bilinmeyen ad AYNEN geçer: uydurma yapmaktansa PPD'nin kendi hata
        // mesajını kullanıcıya göstermek doğru.
        assert_eq!(ppd_adi("balanced-performance"), "balanced-performance");
    }

    #[test]
    fn hata_mesajlari_bos_degil() {
        let e = apply(&Action::ChargeLimit(0)).unwrap_err();
        assert!(!e.to_string().is_empty());
    }
}
