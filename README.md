# guest-test-linux

## Usage

list available configurations:

```bash
cargo xtask list
```

build and run a specific configuration, e.g., `arm64-qemu`:

```bash
cargo xtask build arm64-qemu
```

Kernel and rootfs will be built in `build/arm64-qemu/` dir.
