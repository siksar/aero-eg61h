#!/usr/bin/env bash
# platform_profile katmanının doğrulaması (ADIM 5).
#
# Tek soruyu cevaplar: modül yüklenince
# /sys/firmware/acpi/platform_profile_choices DEĞİŞİYOR MU?
#
# Değişmemeli. Değişirse `low-power` kaybolabilir, o da
# ~/nixos-zixar/system/kernel/power-display.nix'in pildeki power-saver
# otomatiğini kırar ve 4.28 W boşta güç bütçesini vurur.
#
# Kullanım:  sudo bash scripts/verify-profile.sh
#
# Koşu sonunda her şeyi başladığı hâle bırakır (profil, PPD, fan modu, şarj).
set -u
echo "### modulsuz taban (aero kaldiriliyor)"
rmmod aero_eg61h 2>/dev/null; sleep 1
BASE_CH="$(cat /sys/firmware/acpi/platform_profile_choices)"
BASE_PR="$(cat /sys/firmware/acpi/platform_profile)"
echo "  legacy choices : $BASE_CH"
echo "  legacy profile : $BASE_PR"

echo; echo "### yeni modul yukleniyor (kume = amd-pmf ile birebir)"
insmod /home/zixar/aero-eg61h/kernel/aero-eg61h.ko; sleep 1
AFT_CH="$(cat /sys/firmware/acpi/platform_profile_choices)"
echo "  legacy choices : $AFT_CH"
echo "  legacy profile : $(cat /sys/firmware/acpi/platform_profile)"
for d in /sys/class/platform-profile/*/; do
  echo "    $(basename $d) $(cat $d/name): profile=$(cat $d/profile) choices=$(cat $d/choices)"
done

echo; echo "### KRITIK: legacy secenekler modulsuz hale gore DEGISTI mi?"
if [ "$BASE_CH" = "$AFT_CH" ]; then
  echo "  >> BIREBIR AYNI  ($AFT_CH)"
else
  echo "  >> FARKLI: '$BASE_CH' -> '$AFT_CH'"
fi

echo; echo "### uc profil de yaziliyor (legacy dugumden, her iki handler'a)"
for p in low-power performance balanced; do
  echo "$p" > /sys/firmware/acpi/platform_profile 2>/dev/null || { echo "  $p yazilamadi"; continue; }
  sleep 1
  printf '  %-12s -> legacy=%-12s amd-pmf=%-12s aero=%s\n' "$p" \
    "$(cat /sys/firmware/acpi/platform_profile)" \
    "$(cat /sys/class/platform-profile/platform-profile-0/profile)" \
    "$(for d in /sys/class/platform-profile/*/; do [ "$(cat $d/name)" = aero_eg61h ] && cat $d/profile; done)"
done

echo; echo "### sunmadigimiz profil legacy'de var mi (olmamali)"
cat /sys/firmware/acpi/platform_profile_choices | grep -qw balanced-performance \
  && echo "  balanced-performance HALA VAR (hata)" || echo "  balanced-performance YOK (dogru)"

echo; echo "### GERI ALMA"
echo "$BASE_PR" > /sys/firmware/acpi/platform_profile 2>/dev/null
powerprofilesctl set balanced 2>/dev/null
echo "  legacy=$(cat /sys/firmware/acpi/platform_profile)  PPD=$(powerprofilesctl get)"
F=$(echo /sys/bus/wmi/devices/ABBC0F75-*/fan_mode); echo "  fan_mode=$(cat $F)  sarj=$(cat /sys/class/power_supply/BAT1/charge_control_end_threshold)%"
echo "### bitti"
