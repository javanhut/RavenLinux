#!/bin/bash
# =============================================================================
# kernel-virtualization.sh -- VM and container host support for the Raven kernel
# =============================================================================
#
# Applies the Kconfig options that let a Raven machine run virtual machines
# and containers, not just be one. The guest side (virtio, paravirt clock,
# the QEMU/VMware display drivers) was already in the config; what was
# missing was the host side: no /dev/kvm, so QEMU fell back to TCG emulation;
# no tun, vhost, bridge or veth, so neither a VM nor a container could get a
# network; and no BPF syscall, so cgroup v2 device control -- which crun and
# runc both require -- was not there to ask for.
#
# Usage: scripts/kernel-virtualization.sh <kernel-source-dir>
#
# Same contract as kernel-ports.sh and kernel-performance.sh: edits
# <kernel-source-dir>/.config in place with the kernel's own scripts/config,
# and the caller runs `make olddefconfig` afterwards so dependencies resolve.
# Idempotent. build-kernel.sh applies it both when it generates a config from
# scratch and when it restores the saved one.
#
# Module vs built-in: anything that only exists to back a /dev node a program
# opens by path (/dev/net/tun, /dev/vhost-net, /dev/vhost-vsock) is built in,
# because a module behind a device node loads only if something created the
# node first. Everything else is a module that the kernel or udev loads on
# first use, so a machine that never starts a VM pays disk and nothing else.
# =============================================================================

set -euo pipefail

src="$(cd "${1:?usage: $0 <kernel-source-dir>}" && pwd)"
cfg="${src}/scripts/config"
[ -x "$cfg" ] || { echo "no scripts/config in $src" >&2; exit 1; }
cd "$src"

y() { for o in "$@"; do "$cfg" --enable "$o"; done; }
m() { for o in "$@"; do "$cfg" --module "$o"; done; }

# --- KVM: hardware-accelerated guests -----------------------------------------
# Modules: kvm_intel and kvm_amd carry x86cpu modaliases for VMX and SVM, so
# udev's coldplug loads the right one on a CPU that has the extension and
# neither on one that does not. Loading it is what creates /dev/kvm. KVM_SMM
# (OVMF secure boot) and KVM_HYPERV (Windows guests) default on with KVM.
# KVM_WERROR is not set: it follows WERROR, which build-kernel.sh turns off.
m KVM KVM_INTEL KVM_AMD

# Interrupt remapping is what makes device passthrough safe: without it a
# passed-through device can raise any interrupt on the host. x2APIC is
# already on, and on large machines it wants IRQ_REMAP as well.
y IRQ_REMAP

# Kernel samepage merging. QEMU marks guest RAM mergeable with madvise, so
# several guests running the same OS share identical pages. Nothing scans
# until /sys/kernel/mm/ksm/run is set to 1, so it costs nothing until then.
y KSM

# --- vhost and vsock: virtio backends in the host kernel ----------------------
# TUN is the tap device QEMU attaches a guest NIC to, and the device that
# rootless Podman's pasta and slirp4netns networking runs over. VHOST_NET
# moves the virtio-net data path out of QEMU's userspace loop and into the
# kernel. MACVTAP attaches a guest directly to a physical NIC, with no
# bridge. MACVTAP selects TAP, and VHOST_NET cannot be built in while TAP is
# a module, so the three are built in together.
y TUN VHOST_NET MACVLAN MACVTAP

# vsock: a host<->guest socket that needs no network configuration at all
# (qemu-guest-agent, systemd's ssh-over-vsock, container VMs). VHOST_VSOCK
# is the host end. VIRTIO_VSOCKETS is the guest end, for Raven running as
# the guest.
y VSOCKETS VHOST_VSOCK VIRTIO_VSOCKETS

# --- Virtual networks: bridges, veth pairs, NAT -------------------------------
# A bridge is what libvirt's default network (virbr0) and Podman's and
# Docker's default networks are. A veth pair is how a container's network
# namespace is connected to that bridge. Both are modules: `ip link add type
# bridge|veth` makes the kernel request the module itself.
m BRIDGE VETH

# The "nat" and "masquerade" nftables expressions, which libvirt's and
# netavark's nftables backends use to give that bridge outbound access.
# NF_NAT and NF_NAT_MASQUERADE are already built in for the firewall, and
# these two are only the nf_tables front ends. nf_tables loads each one by
# name the first time a rule uses it, the same as NFT_LOG and NFT_LIMIT.
m NFT_NAT NFT_MASQ

# --- Device passthrough -------------------------------------------------------
# VFIO hands a PCI device (a second GPU, a NIC, a USB controller) to a guest
# through the IOMMU. The IOMMU drivers are already built. Intel's is off by
# default (INTEL_IOMMU_DEFAULT_ON is not set), so on Intel, passthrough also
# needs intel_iommu=on on the command line. Modules: vfio-pci is only bound
# by hand or by libvirt, never at boot.
m VFIO VFIO_PCI

# --- Containers: cgroup BPF --------------------------------------------------
# cgroup v2 has no devices.allow file. A container runtime controls which
# device nodes a container may open by attaching a BPF program to the cgroup,
# so crun and runc both need BPF_SYSCALL and CGROUP_BPF and fail to start a
# container without them. Unprivileged BPF stays off by default
# (kernel.unprivileged_bpf_disabled=2: root can set it to 1 but never back to
# 0 without a reboot), and the JIT is always on, so there is no BPF
# interpreter to use as a gadget.
y BPF_SYSCALL CGROUP_BPF BPF_UNPRIV_DEFAULT_OFF
y BPF_JIT BPF_JIT_ALWAYS_ON

# --- Guest side: virtiofs ----------------------------------------------------
# virtiofs is how current QEMU, libvirt and every container-VM runtime share
# a host directory with a guest. It replaces 9p, which stays on for older
# hosts. FUSE_FS is already built in.
y VIRTIO_FS
