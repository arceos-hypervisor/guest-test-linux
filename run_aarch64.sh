qemu-system-aarch64 \
    -machine virt,virtualization=true,gic-version=3 \
    -nographic \
    -m size=1024M \
    -cpu cortex-a72 \
    -smp 2 \
    -kernel build/arm64/linux/arch/arm64/boot/Image \
    -drive format=raw,file=build/arm64/rootfs.img \
    -append "root=/dev/vda rw init=/init"
