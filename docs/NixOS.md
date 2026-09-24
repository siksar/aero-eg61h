# NixOS installation

This guide installs `aero-eg61h` declaratively on a NixOS configuration. It is
separate from the imperative distribution instructions in the root README.
The module is hardware-specific and should be enabled only on a Gigabyte AERO
X16 1VH (EG61VH).

## Add the source

For a flake-based configuration, add the repository as a non-flake input:

```nix
inputs.aero-eg61h = {
  url = "git+file:///home/you/src/aero-eg61h";
  flake = false;
};
```

Import the module and enable it from the host configuration:

```nix
{ inputs, ... }:
{
  imports = [ "${inputs.aero-eg61h}/nix/aero-eg61h.nix" ];
  hardware.aero-eg61h.enable = true;
}
```

For a remote release, replace the URL with the release tarball or a pinned
GitHub source and update the lock file. Pinning is recommended for reproducible
kernel modules.

## Options

The module provides:

- `hardware.aero-eg61h.enable`
- `hardware.aero-eg61h.fanMode.ac`
- `hardware.aero-eg61h.fanMode.battery`
- `hardware.aero-eg61h.fanMode.game`
- `hardware.aero-eg61h.chargeLimit`
- `hardware.aero-eg61h.gpuBoost.ac`
- `hardware.aero-eg61h.gpuBoost.battery`

Defaults preserve the measured reference setup: balanced fan mode on AC and
battery, a 60% charge limit, and the documented GPU boost values. Review the
module before changing these values; the firmware tables are not guaranteed on
another BIOS/EC revision.

## Build, activate, and verify

```bash
sudo nixos-rebuild build --flake .#your-host
sudo nixos-rebuild switch --flake .#your-host
journalctl -k -b | grep aero_eg61h
systemctl status aero-power-profile aero-charge-limit aero-set-fan@balanced
```

The module compiles and loads the kernel module, installs the
charge/fan/profile bridge services, and installs their polkit allow-list. All
EC writes, including the Dynamic Boost budget, go through the kernel driver;
`acpi_call` is not loaded. The polkit rule applies only to the configured user
in a local, active session; remote sessions fall back to administrator
authentication. Reboot after the first switch
so boot ordering is tested, rather than relying only on a manually inserted
module.

## Optional GUI

Build the GUI from the same checkout after activating the kernel module:

```bash
cd app
./build.sh
./run.sh
```

The GUI reads the same sysfs interfaces as `aero-ctl`; it does not replace the
NixOS module or install a background daemon.
