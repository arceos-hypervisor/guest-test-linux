#!/bin/bash

qemu-system-x86_64 \
    -machine q35 \
    -nographic \
    -m size=1024M \
    -cpu qemu64 \
    -smp 2 \
    -kernel build/x86/linux/arch/x86/boot/bzImage \
    -drive format=raw,file=build/x86/rootfs.img \
    -append "root=/dev/sda rw init=/init console=ttyS0"
