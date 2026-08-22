# =============================================================================
# RavenLinux Makefile
# =============================================================================
# Builds a Docker/Podman image containing the RavenLinux build toolchain, and
# runs the build inside it -- so you can produce a RavenLinux ISO from ANY host
# (macOS, Windows, or Linux) without the host itself being Linux.
#
# RavenLinux is a Linux-From-Scratch style distro: the build performs chroot,
# overlayfs mounts, loop-device setup and runs a musl cross-toolchain. None of
# that works natively on macOS/Windows, so the image provides the Linux host.
#
# Everything here delegates to scripts/docker-build.sh, which auto-detects the
# container engine, builds the image (cached), and runs it with the required
# --privileged flag and the repo bind-mounted at /raven. All artifacts (the
# ISO, toolchain, sysroot) land in ./build/ on your host.
#
# Quick start:
#   make image      # build just the toolchain image
#   make build      # build everything and produce the ISO  (-> ./build/)
#   make shell      # open an interactive shell in the build environment
#   make help       # list every target
#
# Common overrides (see "Configuration" below):
#   make build JOBS=8            # 8 parallel compile jobs
#   make iso ARCH=x86_64         # target architecture
#   make build ENGINE=podman     # force Podman instead of Docker
#   make image IMAGE=raven:dev   # custom image tag
# =============================================================================

# ----------------------------------------------------------------------------
# Configuration (override on the command line, e.g. `make build JOBS=8`)
# NOTE: keep these as plain assignments -- inline `#` comments would leak
# trailing whitespace into the values, so the comments live above each one.
# ----------------------------------------------------------------------------
# Container image tag.
IMAGE  ?= ravenlinux-build
# Container engine: docker | podman (empty = auto-detect).
ENGINE ?=
# Parallel build jobs (empty = nproc inside the container).
JOBS   ?=
# Target architecture, e.g. x86_64 (empty = build.sh default).
ARCH   ?=

# The containerized build helper does all the heavy lifting.
DOCKER_BUILD := ./scripts/docker-build.sh

# Pass engine/image selection through to the helper via its env vars.
ENV := RAVEN_IMAGE='$(IMAGE)' $(if $(strip $(ENGINE)),RAVEN_ENGINE='$(ENGINE)',)
RUN := $(ENV) $(DOCKER_BUILD)

# Extra flags forwarded to scripts/build.sh inside the container.
BUILD_FLAGS := $(if $(strip $(JOBS)),-j $(JOBS),) $(if $(strip $(ARCH)),-a $(ARCH),)

.DEFAULT_GOAL := help

# ----------------------------------------------------------------------------
# Image
# ----------------------------------------------------------------------------
.PHONY: image
image: ## Build the Docker/Podman toolchain image only
	$(RUN) image

# ----------------------------------------------------------------------------
# Full build
# ----------------------------------------------------------------------------
.PHONY: build all
build all: ## Build everything and generate the ISO (-> ./build/)
	$(RUN) $(BUILD_FLAGS) all

.PHONY: rebuild
rebuild: ## Clean rebuild from scratch (wipes build dir, then builds all)
	$(RUN) $(BUILD_FLAGS) --clean all

# ----------------------------------------------------------------------------
# RavenLinux container image (the OS itself — for running and testing)
# ----------------------------------------------------------------------------
# Packages the built rootfs (build/sysroot) as a runnable image. This is
# RavenLinux, not the Arch builder. Run a build first, then:
#   make rootfs
#   docker run --rm -it --platform linux/amd64 ravenlinux
ROOTFS_IMAGE ?= ravenlinux:latest

.PHONY: rootfs
rootfs: ## Package the RavenLinux rootfs as image '$(ROOTFS_IMAGE)'
	$(ENV) RAVEN_ROOTFS_IMAGE='$(ROOTFS_IMAGE)' ./scripts/export-rootfs.sh

# ----------------------------------------------------------------------------
# Individual stages (see scripts/build.sh for details)
# ----------------------------------------------------------------------------
.PHONY: stage0
stage0: ## Build the musl cross-compilation toolchain
	$(RUN) $(BUILD_FLAGS) stage0

.PHONY: stage1
stage1: ## Build the base system with the cross toolchain
	$(RUN) $(BUILD_FLAGS) stage1

.PHONY: stage2
stage2: ## Native rebuild of the entire system
	$(RUN) $(BUILD_FLAGS) stage2

.PHONY: stage3
stage3: ## Build base packages (core libraries, shells, OpenSSH)
	$(RUN) $(BUILD_FLAGS) stage3

.PHONY: stage4 iso
stage4 iso: ## Generate the bootable ISO from existing build output
	$(RUN) $(BUILD_FLAGS) stage4

# ----------------------------------------------------------------------------
# Interactive
# ----------------------------------------------------------------------------
.PHONY: shell
shell: ## Open an interactive shell inside the build environment
	$(RUN) shell

# ----------------------------------------------------------------------------
# Cleaning
# ----------------------------------------------------------------------------
.PHONY: clean
clean: ## Remove host build artifacts (./build)
	@echo ">> Removing build artifacts in ./build ..."
	@rm -rf build 2>/dev/null || { \
		echo "!! Some artifacts are root-owned (created by the privileged container)."; \
		echo "!! Remove them with: sudo rm -rf build"; \
		exit 1; }
	@echo ">> Done."

.PHONY: clean-image
clean-image: ## Remove the toolchain image ($(IMAGE))
	@echo ">> Removing image '$(IMAGE)' ..."
	@{ command -v docker >/dev/null 2>&1 && docker rmi -f '$(IMAGE)'; } \
		|| { command -v podman >/dev/null 2>&1 && podman rmi -f '$(IMAGE)'; } \
		|| echo "!! No docker/podman found, or image not present."

.PHONY: distclean
distclean: clean clean-image ## Remove build artifacts AND the toolchain image

# ----------------------------------------------------------------------------
# Help
# ----------------------------------------------------------------------------
.PHONY: help
help: ## Show this help
	@echo "RavenLinux build (containerized -- runs on macOS, Windows, or Linux)"
	@echo ""
	@echo "Usage: make <target> [JOBS=N] [ARCH=x86_64] [ENGINE=docker|podman] [IMAGE=tag]"
	@echo ""
	@echo "Targets:"
	@grep -E '^[a-zA-Z0-9 _-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-13s\033[0m %s\n", $$1, $$2}'
