//! Klavye aydınlatması — `kbd-rgb` aracının GUI yüzü.
//!
//! NEDEN ALT SÜREÇ, NEDEN KENDİ ioctl'İMİZ DEĞİL: LampArray protokolü bu
//! depoda değil, nixos-zixar'da yaşıyor
//! (`system/drivers/input/keyboard-rgb/src/main.rs`). Burada ikinci bir kopya
//! açmak, aynı gerçeği iki eve koymak olurdu — ve ikincisi kaçınılmaz olarak
//! eskir. GUI yalnız komutu çağırıyor, durumu `status --json` ile geri okuyor.
//!
//! NEDEN `aero-sysfs` DEĞİL: o modülün sözleşmesi "yalnız `aero_eg61h`
//! sürücüsünün sysfs yüzeyi". Klavye ışığı bir HID cihazı, sysfs değil; oraya
//! koymak modülün tanımını bulanıklaştırırdı.

use std::process::Command;

/// `kbd-rgb status --json` çıktısının okunmuş hâli.
///
/// `cihaz` `None` ise klavye takılı değil (ya da hidraw düğümü görünmüyor);
/// panel bu durumda kontrolleri kapatıp bir açıklama gösteriyor. Araç cihaz
/// yokken de çalışıp `device:null` döndürdüğü için burada hata yolu yok.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Durum {
    pub cihaz: Option<String>,
    /// Altı haneli hex, "#" YOK — kbd-rgb'nin kendi biçimi.
    pub renk: String,
    /// 0..=100. Donanımda ayrı parlaklık kanalı yok; kbd-rgb RGB'yi ölçekliyor.
    pub parlaklik: u32,
    pub acik: bool,
    /// Dönen animasyonun adı (`breathe` / `rainbow`), yoksa `None`.
    /// Bu bilgi JSON'da DEĞİL — systemd'den ayrıca okunuyor, çünkü animasyonu
    /// kbd-rgb değil `kbd-rgb-anim@.service` yönetiyor.
    pub animasyon: Option<String>,
}

/// kbd-rgb'nin ön ayar adları (`main.rs` içindeki `PRESETS` ile aynı sıra).
/// Panelde hızlı renk düğmeleri olarak çiziliyor; hex girişi de serbest.
pub const ON_AYAR_RENKLER: &[(&str, &str)] = &[
    ("Red", "ff0000"),
    ("Orange", "ff7800"),
    ("Yellow", "ffff00"),
    ("Green", "00ff00"),
    ("Cyan", "00ffff"),
    ("Blue", "0000ff"),
    ("Purple", "a000ff"),
    ("Pink", "ff50a0"),
    ("White", "ffffff"),
];

pub const ANIMASYONLAR: &[(&str, &str)] = &[("Breathe", "breathe"), ("Rainbow", "rainbow")];

/// Panelin donanıma yaptırabileceği her şey. `update()` bunları tek bir yerden
/// geçiriyor ki hata yolu da tek olsun.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eylem {
    /// Altı haneli hex (# olmadan).
    Renk(String),
    /// 0..=100.
    Parlaklik(u32),
    /// Yanıyorsa söndür, sönükse son renkle yak.
    Toggle,
    /// `Some(mod)` başlatır/değiştirir, `None` çalışanı durdurur.
    Animasyon(Option<String>),
    /// Oturum açılışındaki tema servisini yeniden koşturur → Stylix rengi.
    StylixDon,
}

/// JSON'dan tek bir alanı çeker.
///
/// Elle yazıldı çünkü bu crate'in bağımlılık listesinde serde YOK ve tek bir
/// düz nesne için eklemek ağır olurdu. Girdi bizim kendi aracımızın çıktısı:
/// iç içe nesne, dizi, kaçış dizisi ya da boşluk içermiyor.
fn alan<'a>(json: &'a str, ad: &str) -> Option<&'a str> {
    let anahtar = format!("\"{ad}\":");
    let bas = json.find(&anahtar)? + anahtar.len();
    let kalan = &json[bas..];
    let son = kalan.find([',', '}'])?;
    Some(kalan[..son].trim().trim_matches('"'))
}

fn kbd_rgb(argumanlar: &[&str]) -> Result<String, String> {
    let cikti = Command::new("kbd-rgb")
        .args(argumanlar)
        .output()
        .map_err(|e| format!("kbd-rgb çalıştırılamadı: {e}"))?;
    if !cikti.status.success() {
        return Err(String::from_utf8_lossy(&cikti.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&cikti.stdout).into_owned())
}

/// Dönen animasyonu systemd'ye sorar.
///
/// İki modu tek tek yokluyor; `list-units` çıktısını ayrıştırmaktan daha
/// dayanıklı, çünkü birim adı biçimi değişse bile burası ya bulur ya bulmaz.
fn animasyon_oku() -> Option<String> {
    ANIMASYONLAR.iter().find_map(|(_, m)| {
        let birim = format!("kbd-rgb-anim@{m}.service");
        let cikti = Command::new("systemctl")
            .args(["--user", "is-active", &birim])
            .output()
            .ok()?;
        // `is-active` etkin değilken 3 ile çıkar; stdout'a bakmak yeterli.
        String::from_utf8_lossy(&cikti.stdout)
            .trim()
            .eq("active")
            .then(|| (*m).to_string())
    })
}

impl Durum {
    /// Donanıma DOKUNMADAN durumu okur. Araç bulunamazsa varsayılan (cihaz
    /// yok) dönüyor — panel o hâlde kontrolleri kapatıp açıklama gösteriyor.
    pub fn oku() -> Self {
        let Ok(json) = kbd_rgb(&["status", "--json"]) else {
            return Self::default();
        };
        Self {
            cihaz: alan(&json, "device")
                .filter(|v| *v != "null")
                .map(str::to_owned),
            renk: alan(&json, "color").unwrap_or("000000").to_owned(),
            parlaklik: alan(&json, "brightness")
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            acik: alan(&json, "on") == Some("true"),
            animasyon: animasyon_oku(),
        }
    }

    pub fn var(&self) -> bool {
        self.cihaz.is_some()
    }
}

/// Eylemi uygular. Hata metni kullanıcıya AYNEN gösteriliyor (yutulmuyor) —
/// panelin hata şeridi deseni fan/şarj tarafıyla aynı.
pub fn uygula(eylem: &Eylem) -> Result<(), String> {
    match eylem {
        Eylem::Renk(hex) => kbd_rgb(&["set", hex]).map(drop),
        Eylem::Parlaklik(pct) => kbd_rgb(&["bright", &pct.to_string()]).map(drop),
        Eylem::Toggle => kbd_rgb(&["toggle"]).map(drop),

        // Animasyonu systemd yönetiyor: şablon birim, `wantedBy` yok, boşta
        // dönmüyor. Mod değiştirmek için önce çalışanı durdurmak gerekiyor —
        // iki döngü aynı lambayı çekiştirirse titreme olur.
        Eylem::Animasyon(hedef) => {
            for (_, m) in ANIMASYONLAR {
                if Some(*m) != hedef.as_deref() {
                    let _ = Command::new("systemctl")
                        .args(["--user", "stop", &format!("kbd-rgb-anim@{m}.service")])
                        .output();
                }
            }
            match hedef {
                Some(m) => Command::new("systemctl")
                    .args(["--user", "start", &format!("kbd-rgb-anim@{m}.service")])
                    .output()
                    .map_err(|e| format!("animasyon başlatılamadı: {e}"))
                    .and_then(|o| {
                        o.status
                            .success()
                            .then_some(())
                            .ok_or_else(|| String::from_utf8_lossy(&o.stderr).trim().to_string())
                    }),
                None => Ok(()),
            }
        }

        // Renk eval zamanında Stylix paletinden gömülü; servisi yeniden
        // koşturmak onu geri yazıyor. Renk burada BİLİNMİYOR ve bilinmemeli —
        // tek kaynak lib/theme.nix.
        Eylem::StylixDon => Command::new("systemctl")
            .args(["--user", "restart", "kbd-rgb-theme.service"])
            .output()
            .map_err(|e| format!("tema servisi çağrılamadı: {e}"))
            .and_then(|o| {
                o.status
                    .success()
                    .then_some(())
                    .ok_or_else(|| String::from_utf8_lossy(&o.stderr).trim().to_string())
            }),
    }
}

/// Hex girişini doğrular: altı hane, isteğe bağlı '#'. Panel yazmadan önce
/// buna bakıyor, böylece geçersiz giriş alt sürece hiç gitmiyor.
pub fn hex_temizle(giris: &str) -> Option<String> {
    let t = giris.trim().trim_start_matches('#').to_ascii_lowercase();
    (t.len() == 6 && t.chars().all(|c| c.is_ascii_hexdigit())).then_some(t)
}

#[cfg(test)]
mod testler {
    use super::*;

    #[test]
    fn alan_cikarimi() {
        let j = r#"{"device":"/dev/hidraw9","lamps":1,"kind":6,"color":"8ba4b0","brightness":50,"on":true}"#;
        assert_eq!(alan(j, "device"), Some("/dev/hidraw9"));
        assert_eq!(alan(j, "color"), Some("8ba4b0"));
        assert_eq!(alan(j, "brightness"), Some("50"));
        assert_eq!(alan(j, "on"), Some("true"));
        assert_eq!(alan(j, "yok"), None);
    }

    #[test]
    fn cihaz_yokken_null() {
        let j = r#"{"device":null,"lamps":0,"kind":0,"color":"8ba4b0","brightness":50,"on":true}"#;
        assert_eq!(alan(j, "device"), Some("null"));
    }

    #[test]
    fn hex_dogrulama() {
        assert_eq!(hex_temizle("#8BA4B0"), Some("8ba4b0".into()));
        assert_eq!(hex_temizle(" 8ba4b0 "), Some("8ba4b0".into()));
        assert_eq!(hex_temizle("8ba4b"), None);
        assert_eq!(hex_temizle("zzzzzz"), None);
    }
}
