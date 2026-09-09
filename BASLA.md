# Release hand-off

Start with `README.md`. The repository is self-contained; it does not modify a
separate NixOS checkout automatically. Build the userspace workspace with
`cd app && cargo test --workspace`, then use `aero-ctl` before launching the
GUI. Read `docs/NixOS.md` before enabling the declarative module.

The hardware-specific safety rules are important: use only an EG61VH, do not
load two WMI writers, and do not bypass the DMI gate on production machines.
