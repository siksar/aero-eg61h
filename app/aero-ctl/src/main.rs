// SPDX-License-Identifier: GPL-2.0-only
//! `aero-ctl` — durum dökümü.
//!
//! GUI'nin okuduğu her şeyi uçbirimde gösterir. İki işi var:
//!   1. `aero-sysfs`'in koşturulabilir testi — GUI'yi beklemeden doğrulanır.
//!   2. Tanılama: bir şey görünmüyorsa sebebi burada yazıyor.
//!
//! Yetki İSTEMEZ — okuduğu dört değerin dördü de dünyaya-okunur.

use aero_sysfs::{Action, FanMode, Snapshot, apply};
use std::path::Path;

fn kullanim() -> ! {
    eprintln!(
        "usage:
  aero-ctl [--root PATH]         print a status snapshot (default)
  aero-ctl set fan <mod>         quiet | balanced | responsive | gaming | turbo
  aero-ctl set charge <1-100>    charge limit percentage
  aero-ctl set profile <ad>      low-power | balanced | performance

Writes use the existing polkit-authorized systemd units and PPD.
Validation happens in the privileged unit; this check only provides fast
feedback."
    );
    std::process::exit(2);
}

fn set(args: &[String]) -> ! {
    let (ne, deger) = match args {
        [ne, deger] => (ne.as_str(), deger.as_str()),
        _ => kullanim(),
    };

    let action = match ne {
        "fan" => match FanMode::from_sysfs(deger) {
            Some(m) => Action::FanMode(m),
            None => {
                eprintln!("unknown fan mode: {deger}");
                eprintln!("valid values: quiet balanced responsive gaming turbo");
                std::process::exit(2);
            }
        },
        "charge" => match deger.parse::<u8>() {
            Ok(v) => Action::ChargeLimit(v),
            Err(_) => {
                eprintln!("not a number: {deger}");
                std::process::exit(2);
            }
        },
        "profile" => Action::Profile(deger.to_string()),
        _ => kullanim(),
    };

    match apply(&action) {
        Ok(()) => {
            // Ne yazdigimizi degil, sistemin GERCEKTEN ne okudugunu bildir.
            // (the legacy driver's hatasi tam olarak yazdigini bildirmekti.)
            let s = Snapshot::read();
            match ne {
                "fan" => println!("fan mode: {}", s.fan_mode.map_or("—".into(), |m| m.as_sysfs().to_string())),
                "charge" => println!("charge limit: %{}", s.charge_limit_pct.map_or("—".into(), |v| v.to_string())),
                _ => println!("profile: {}", s.platform_profile.unwrap_or_else(|| "—".into())),
            }
            std::process::exit(0)
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1)
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().map(String::as_str) == Some("set") {
        set(&args[1..]);
    }
    if args.iter().any(|a| a == "-h" || a == "--help") {
        kullanim();
    }

    // `--root DIZIN`: sahte bir sysfs agacina karsi calistirir. Tanilama ve
    // "surucu yokken ne diyor" yolunu makineye dokunmadan gormek icin.
    let s = match args.iter().position(|a| a == "--root") {
        Some(i) if args.len() > i + 1 => Snapshot::read_from(Path::new(&args[i + 1])),
        _ => Snapshot::read(),
    };


    println!("AERO X16 1VH (EG61VH) — status");
    println!();

    if !s.problems.is_empty() {
        println!("  ⚠ MISSING CAPABILITIES");
        for p in &s.problems {
            println!("    {} — {}", p.what, p.why);
        }
        println!();
    }

    let na = "—".to_string();

    println!("  Driver      : {}", if s.driver_present { "aero_eg61h loaded" } else { "NOT LOADED" });
    println!("  Power source: {}", match s.ac_online {
        Some(true) => "AC",
        Some(false) => "Battery",
        None => "—",
    });
    println!();

    println!("  CPU         : {}", s.cpu_temp_c.map(|v| format!("{v} °C")).unwrap_or_else(|| na.clone()));
    println!("  Fan 1       : {}", s.fan1_rpm.map(|v| format!("{v} rpm")).unwrap_or_else(|| na.clone()));
    println!("  Fan 2       : {}", s.fan2_rpm.map(|v| format!("{v} rpm")).unwrap_or_else(|| na.clone()));
    println!();

    match (s.fan_mode, &s.fan_mode_raw_unknown) {
        (Some(m), _) => println!("  Fan mode    : {} ({}) — {}", m.label(), m.as_sysfs(), m.describe()),
        (None, Some(raw)) => println!("  Fan mode    : UNKNOWN (`{raw}`)"),
        (None, None) => println!("  Fan mode    : {na}"),
    }
    if !s.fan_mode_choices.is_empty() {
        let list: Vec<_> = s.fan_mode_choices.iter().map(|m| m.as_sysfs()).collect();
        println!("    options: {}", list.join(" "));
    }
    println!();

    println!("  Profile     : {}", s.platform_profile.clone().unwrap_or_else(|| na.clone()));
    if !s.platform_profile_choices.is_empty() {
        println!("    options: {}", s.platform_profile_choices.join(" "));
    }
    if s.platform_profile.as_deref() == Some("custom") {
        println!("    (custom = the driver has not written a profile; WMBC has no 0xED read,");
        println!("     so the EC state is unknown and is not guessed)");
    }
    println!();

    println!("  Battery     : {}", s.battery_pct.map(|v| format!("%{v}")).unwrap_or_else(|| na.clone()));
    println!("  Charge limit: {}", s.charge_limit_pct.map(|v| format!("%{v}")).unwrap_or_else(|| na.clone()));
}
