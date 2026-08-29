# Compatibility shim. RavenLinux is built with ImLazy; this remains so old
# automation and muscle memory fail forward to the canonical interface.
.DEFAULT_GOAL := help

.PHONY: help
help:
	@command -v imlazy >/dev/null 2>&1 || { echo "imlazy is required; install Raven's ImLazy task runner first."; exit 1; }
	@imlazy help

.PHONY: image build all rebuild rootfs layout skeleton manifests stage0 stage1 stage2 stage3 raven gui stage4 iso initramfs dev dev-diff dev-all test qemu qemu-uefi qemu-desktop smoke shell clean clean-image distclean
image build all rebuild rootfs layout skeleton manifests stage0 stage1 stage2 stage3 raven gui stage4 iso initramfs dev dev-diff dev-all test qemu qemu-uefi qemu-desktop smoke shell clean clean-image distclean:
	@command -v imlazy >/dev/null 2>&1 || { echo "imlazy is required; install Raven's ImLazy task runner first."; exit 1; }
	@imlazy $@
