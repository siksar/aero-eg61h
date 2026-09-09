# NixOS integration

`aero-eg61h.nix` is the declarative module used by the NixOS guide in
[`docs/NixOS.md`](../docs/NixOS.md). It builds and loads the hardware-specific
kernel module, installs the measured bridge services and polkit rule, and
prevents a conflicting vendor module from claiming the EC WMI methods.

The module defaults to balanced fan mode and a 60% charge limit. Inspect all
options before changing them on a different BIOS/EC revision. The module does
not install a daemon and does not modify files outside the host configuration.
