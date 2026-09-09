# aero-eg61h

`aero-eg61h` is an independent Linux control stack for the **Gigabyte AERO X16
1VH (EG61VH)**. It contains a small kernel WMI driver, a Rust sysfs/command-line
client, and an optional COSMIC desktop application. The implementation was
written for this EC/BIOS combination from DSDT and firmware measurements; it
does not copy another laptop driver's source or mode model.

The project deliberately exposes only behavior that has been measured on the
supported machine. The WMI layer has a DMI gate for EG61VH and registers three
separate WMI drivers for the read, write, and event GUIDs. All multi-step fan
writes are serialized and read back through the matching WMI selectors.

## Supported hardware

| Hardware | Support |
| --- | --- |
| Gigabyte AERO X16 1VH, SKU EG61VH | Supported and measured |
| BIOS FB0A / EC F00A | Reference firmware used for the tables |
| Other Gigabyte/AERO models | Not supported; do not use `force=1` |

A different BIOS or EC may change selector meanings and fan tables. The driver
refuses to bind when the DMI identity does not match unless explicitly forced.

## Features

- Read-only CPU temperature and two fan RPM channels through hwmon.
- Five measured fan modes: `quiet`, `balanced`, `responsive`, `gaming`, and
  `turbo`, exposed through a dedicated WMI sysfs attribute.
- Standard `power_supply` charge threshold (`charge_control_end_threshold`).
- Standard `platform_profile` values: `low-power`, `balanced`, and `performance`.
- Rust `aero-ctl` diagnostics and write commands using the installed polkit
  bridges; no always-running daemon is required.
- COSMIC GUI with presets, live status, fan/thermal controls, fan-curve plots,
  power/performance, battery, and an About page. English is the default UI
  language.

## Known limitations

- Fan duty/speed cannot be set directly: firmware duty registers accept writes
  but do not change the measured RPM. The GUI therefore offers fan-mode
  selection, not a misleading speed slider.
- The socket-temperature channel is inactive on this EC and is not exported.
- Keyboard lighting writes had no visible effect and are not exposed.
- Fan curves are firmware-image tables rendered as steps. Interpolation between
  measured thresholds is unknown, and live EC curve validation is unavailable
  because the ACPI timeout state for the `EIDR` mailbox is not exposed.
- The AC/BAT mode tables were measured on the reference firmware. Re-check them
  after an EC or BIOS update.
- The kernel module is hardware-specific and requires a matching kernel build.

## Installation on imperative distributions

These instructions apply to Arch, Fedora, openSUSE, Debian, and similar systems
where the kernel module and services are installed manually. You need a running
kernel with WMI, hwmon, `power_supply`, and `platform_profile` support, kernel
headers, a C compiler/LLVM toolchain, Rust/Cargo, and `systemd`/polkit for the
write bridges.

1. Stop or blacklist any other vendor driver that claims the same EC WMI
   methods. Do not load two writers at once.
2. Build the module against the running kernel. `kernel/build.sh` uses the
   project’s pinned Nix development environment; alternatively run the kernel
   module Makefile with your distribution’s kernel build directory:

   ```bash
   cd kernel
   make -C /lib/modules/$(uname -r)/build M="$PWD" LLVM=1 modules
   ```

3. Load and inspect it:

   ```bash
   sudo insmod kernel/aero-eg61h.ko
   journalctl -k -b | grep aero_eg61h
   cat /sys/class/hwmon/hwmon*/name
   ```

4. Install `aero-eg61h.ko` through your distribution’s preferred DKMS, kmod,
   or packaged-module mechanism. Ensure the module is loaded after the ACPI
   battery provider; the driver retries the battery registration during boot.
5. Install the polkit/systemd bridge units from your distribution integration
   (the service names are documented in `nix/aero-eg61h.nix`). Keep their
   allow-list validation on the privileged side.
6. Build the userspace workspace and run the diagnostic first:

   ```bash
   cd app
   cargo test --workspace
   cargo build --release
   ./target/release/aero-ctl
   ./target/release/aero-ctl set fan balanced
   ./target/release/aero-ctl set charge 60
   ```

   `./run.sh` supplies the runtime library path needed by the COSMIC GUI on
   the reference NixOS environment. On other distributions, start
   `aero-control` normally after installing libcosmic and its Wayland/X11
   runtime dependencies.

To remove a manually loaded module, stop the bridge services, then run
`sudo rmmod aero_eg61h`. Never force the module on unsupported hardware.

## NixOS

The supported NixOS integration is in [`nix/aero-eg61h.nix`](nix/aero-eg61h.nix)
and the standalone guide is [`docs/NixOS.md`](docs/NixOS.md). It builds the
module, prevents conflicting EC writers from loading, installs the bridge units,
and enables the polkit rule. The GUI is built separately from `app/` and can be
launched with `./run.sh` after the NixOS switch.

## Development and validation

```bash
cd app
cargo test --workspace
cargo build --workspace
bash ../scripts/verify-profile.sh   # only on a configured test machine
```

`aero-sysfs` accepts `--root` through `aero-ctl` so its missing-device behavior
can be tested against a fake sysfs tree. The kernel build requires kernel
headers and is intentionally not run by Cargo.

## License

The kernel code is GPL-2.0-only. Rust application files carry the same SPDX
notice where applicable; see the file headers before redistributing binaries.
