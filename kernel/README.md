# aero-eg61h kernel module

This directory contains the independent `aero_eg61h` WMI platform driver for
Gigabyte AERO X16 1VH (EG61VH). It wraps the machine’s WMBC read, WMBD write,
and event GUIDs directly through the Linux WMI API and exposes only standard
Linux interfaces plus the measured `fan_mode` attribute.

## Exported interfaces

```text
/sys/class/hwmon/hwmonN/                     (name=aero_eg61h)
  temp1_input, temp1_label                   CPU temperature, read-only
  fan1_input, fan1_label                     fan 0 RPM, read-only
  fan2_input, fan2_label                     fan 1 RPM, read-only

/sys/class/power_supply/BAT1/
  charge_control_end_threshold               1..100%, read/write

/sys/bus/wmi/devices/ABBC0F75-*/
  fan_mode                                    quiet|balanced|responsive|gaming|turbo
  fan_mode_choices                            read-only list

/sys/class/platform-profile/aero_eg61h/
  profile, choices                            standard platform-profile ABI
```

A zero RPM reading is valid: the quiet and balanced curves stop the fans below
their measured threshold. There is no PWM interface because the underlying duty
registers do not control these fans.

## Build and load

```bash
./build.sh
sudo insmod ./aero-eg61h.ko
journalctl -k -b | grep aero_eg61h
```

The helper script uses the pinned LLVM/kernel environment from the reference
NixOS checkout. On a conventional distribution, the equivalent command is:

```bash
make -C /lib/modules/$(uname -r)/build M="$PWD" LLVM=1 modules
```

The DMI gate accepts only SKU EG61VH. `force=1` bypasses only the DMI gate and
is for controlled debugging only; selector meanings have not been validated on
other models.

The driver refuses to bind when a conflicting vendor platform driver is already
loaded. This prevents two writers from racing over the same WMI methods. Remove
the conflicting module before loading `aero_eg61h`. The separate
`ignore_conflict=1` parameter bypasses this check; do not use it in production.

Besides the fan mode, the WMBD device exposes `dgpu_boost` (WMBD 0x4C, 0-10,
NPCF.ACBT = value × 8 W). The EC has no read-back for it, so the node reports
the last value the driver wrote, or `unknown`. Writing the charge limit also
sets and verifies the measured charge mode (BCPS, WMBD 0x64 = 4) under the same
lock.

## WMI and EC design

WMBC and WMBD return values are not sufficient to prove a write succeeded. The
driver therefore serializes each multi-selector operation and reads the state
back through WMBC. Fan modes are complete four-bit patterns, not independent
bit toggles. Charge-limit writes are read back and rejected if the EC returns a
different value.

The event channel is logged in English, but no unmeasured event action is
attached yet. The `EIDR` curve mailbox is intentionally not exposed: ACPI does
not provide the timeout state needed to distinguish stale data from a valid
reply.
