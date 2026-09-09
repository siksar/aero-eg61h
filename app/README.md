# Userspace

The `app/` directory is a Cargo workspace with three crates:

- **`aero-sysfs`** reads the driver’s optional sysfs surfaces and invokes the
  existing polkit/systemd bridges for writes. Every node is optional; missing
  hardware is represented as a diagnostic rather than a panic.
- **`aero-ctl`** prints a complete English diagnostic snapshot and provides
  `set fan`, `set charge`, and `set profile` commands.
- **`aero-control`** is an English-first COSMIC GUI. It has presets, live status,
  fan/thermal controls, measured step-curve plots, power/performance, battery,
  and About panels. It does not require root.

## Build

```bash
cargo test --workspace
cargo build --release
```

On the reference NixOS checkout, `build.sh` obtains the pinned Rust toolchain
and builds both debug/tests and release artifacts. `run.sh` prepares the runtime
library path required by winit/libcosmic.

## Safety model

The application never writes directly to root-owned sysfs nodes. Fan modes and
charge limits are sent to allow-listed systemd units; profiles use
`powerprofilesctl`. The privileged unit validates every instance argument.
After a successful write, the application reads the corresponding sysfs value
again and reports the value returned by the kernel.

The curve panel uses tables extracted from the reference firmware image. It is
visualization only: changing a curve selector changes no hardware state.
