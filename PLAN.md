# Release scope and design notes

`aero-eg61h` is intentionally conservative. The kernel driver, sysfs reader,
CLI, GUI, and NixOS module are separate layers so each can be tested without a
running desktop session or hardware.

## Invariants

- Only the EG61VH DMI identity is accepted by default.
- Every WMI write is serialized and read back.
- Missing sysfs nodes become English diagnostics, not panics.
- The application does not run as root and does not start a resident daemon.
- No unmeasured fan, temperature, keyboard-light, or EC-debug control is
  presented as supported.
- The GUI and kernel messages are English-first.

## Deliberately deferred

Live EC curve validation remains blocked until an observable ACPI timeout state
for the EIDR mailbox is available. Direct fan-duty control and the inactive
socket-temperature channel are not planned without new measurements. Revisit
these decisions only after collecting evidence on the same firmware revision.

See the root README, `kernel/README.md`, `docs/NixOS.md`, and `nix/` for the
release and installation details.
