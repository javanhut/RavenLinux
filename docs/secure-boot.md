# Secure Boot on RavenLinux

RavenLinux ships with Secure Boot **off**, and every other page of
documentation here says to turn it off. This one is about the other answer:
making the firmware trust *this machine's own key* instead of the vendor's, so
that Secure Boot can be left on and RavenLinux still starts.

It is off by default and it stays off by default. Nothing in an ordinary
install touches the firmware. What follows is opt-in, it is reversible at every
step, and the parts of it that could not be tested against real firmware are
listed at the bottom under **What has not been done**, because a documented gap
is worth more than a confident guess about a machine that will not boot.

---

## Why Raven cannot just be signed like everyone else

The short version: RavenBoot is Raven's own EFI binary, built in this tree, and
it is signed by nobody a firmware has heard of.

Every PC firmware ships with Microsoft's certificates in its `db`. A
distribution that wants to boot under Secure Boot out of the box gets a small
first-stage loader called **shim** signed by Microsoft, and shim then checks
everything after it against a key the distribution controls. That requires an
account with Microsoft, a submission process, and a binary nobody in this tree
has built. RavenBoot is not shim and is not signed by anybody.

So there are exactly two honest answers:

1. **Turn Secure Boot off.** This is the default, it is what the installer
   tells people to do, and it is not a failure. It is the same security posture
   as the overwhelming majority of Linux installations.
2. **Replace the firmware's keys with your own**, and sign RavenBoot and the
   kernel with them. That is what this page is about. It gives a machine a real
   Secure Boot chain that the owner controls end to end — and it is only
   possible on a machine whose owner can get into the firmware setup.

There is no third answer where Raven is signed by somebody else.

---

## What is verified, and what is not

RavenBoot does not hand-roll the Linux boot protocol. `bootloader/src/linux.rs`
loads the kernel with the firmware's own `LoadImage` and `StartImage`, which
means an enforcing firmware checks the kernel image exactly as it checked the
loader. `CONFIG_EFI_STUB=y`, so `vmlinuz` is a PE binary a firmware can verify.

```
  firmware
     |  verifies against db  -->  \EFI\BOOT\BOOTX64.EFI   (RavenBoot)
     |                            or \EFI\raven\raven-boot.efi
     v
  RavenBoot
     |  LoadImage  --> firmware verifies again -->  \EFI\raven\vmlinuz
     |  LoadFile2  --> NOT verified            -->  \EFI\raven\initrd.img
     v
  kernel
     |  module loading: NOT verified (CONFIG_MODULE_SIG is off)
     v
  /sbin/init
```

**Three files have to be signed, and only three:**

| File | Why |
|---|---|
| `\EFI\BOOT\BOOTX64.EFI` | the firmware's removable-media fallback path, which is how RavenLinux boots on most laptop firmware |
| `\EFI\raven\raven-boot.efi` | the same binary, where an NVRAM entry points |
| `\EFI\raven\vmlinuz` | RavenBoot loads it through `LoadImage`, so the firmware checks it too |

A signed loader and an unsigned kernel is a machine that shows its boot menu and
then stops, with no message. That is the most likely way to get this wrong.

**The initramfs is not signed and cannot be.** It is handed to the kernel
through the `LINUX_EFI_INITRD_MEDIA_GUID` / `LoadFile2` protocol, which has no
signature check anywhere in the specification. The kernel is what measures it.

**GRUB is not involved.** The task that led to this work expected a GRUB EFI
binary to sign; there is not one. `raven-install` never installs GRUB.
`configs/grub` holds a theme, `grub.cfg` is generated inside `stage4-iso.sh` for
the live ISO only, and `bootloader/preview/src/grub.rs` says in as many words that GRUB
here is BIOS-only. The EFI binary an installed Raven machine boots is RavenBoot.

---

## Kernel modules are deliberately not signed

`CONFIG_MODULE_SIG` is **off**, and turning it on is not part of this. The
reasoning is written out at the symbol in `configs/kernel/config-6.17-raven` and
in `docs/kernel-hardening.md`; the short form:

- Enabling it needs `MODULE_SIG_ALL`, a hash, and a *persistent* `MODULE_SIG_KEY`
  — the default regenerates `certs/signing_key.pem` on every clean tree, which
  breaks both reproducible builds and every module installed by a previous
  build.
- `install_headers()` would then have to be careful never to ship `certs/` to
  user machines.
- DKMS rebuilds of **evdi** (the DisplayLink dock) and of the NVIDIA modules
  happen *on the target machine*, which has no signing key. With
  `MODULE_SIG_FORCE` that is a dock that stops working; without it, it is a
  taint flag.

So: **Secure Boot here verifies everything up to and including the kernel, and
nothing after it.** That is a smaller guarantee than a distribution with a
signing infrastructure offers. It is a much larger one than nothing, it breaks
nobody's hardware, and it is a far smaller change. Signing modules is a separate
piece of work with its own key-management problem; nothing here forecloses it.

### And the guarantee is smaller than that, for two more reasons worth knowing

- **`CONFIG_SECURITY_LOCKDOWN_LSM` is not set** (config line 7409). Without
  lockdown, root on a running Raven machine can load an unsigned module, write
  `/dev/mem`, or `kexec` (`CONFIG_KEXEC=y`) into a kernel of its choosing. A
  verified boot chain that anything with root can step outside of afterwards
  protects against an attacker who can write to the ESP while the machine is
  off — an "evil maid" — and not against one who already has root.
- **The boot configuration is not signed.** `\EFI\raven\boot.cfg` supplies the
  kernel command line, and `\EFI\raven\snaps.cfg` (written by `raven-snapshot`)
  supplies more of them. Both are plain files on a FAT partition that root can
  rewrite. Secure Boot verifies *binaries*, not the arguments they are given, so
  the signed chain does not stop root from arranging for the next boot to run
  `init=/bin/sh`. That is true of essentially every unified-kernel-less setup;
  it is written down here because it is the thing people assume Secure Boot
  covers and it does not.

  The same sentence covers RavenBoot's `e` key, which opens a prompt that
  appends to the selected entry's command line for one boot — see
  [`boot-recovery.md`](boot-recovery.md). It lowers the cost of supplying an
  argument from "rewrite a file on the ESP" to "press a key", and it changes
  the guarantee not at all, because the guarantee never covered arguments.
  Worth being clear-eyed about it anyway: an attacker at the keyboard of an
  unencrypted machine gets a root shell, and always could — the menu that
  `raven-install` writes has carried a *rescue shell* entry with
  `init=/bin/bash` in it since before the key existed. What actually stops
  that person is full-disk encryption, a firmware password, or both.

Neither of these is a reason not to enrol keys. Both are reasons not to describe
the result as more than it is.

---

## The three states a firmware can be in

This is the part that trips everybody up, because two of the three look
identical from inside Linux until you try to write a key.

| `SecureBoot` | `SetupMode` | What it means | Can Raven enrol keys? |
|---|---|---|---|
| `1` | `0` | Enforcing. RavenBoot is refused before it runs. | No |
| `0` | `0` | Off, but the firmware still holds its vendor's Platform Key. | No |
| `0` | `1` | **Setup mode.** No Platform Key at all; anything may write one. | Yes |

Setup mode is not a state a machine ships in. Somebody has been into the
firmware setup and chosen *Clear Secure Boot Keys* (also spelled *Delete all
Secure Boot variables*, or *Erase all secure boot settings*). That is the whole
reason enrolment does not need a firmware password: with no Platform Key
enrolled, the firmware has nothing to check a new one against.

Both variables are EFI globals and can be read directly:

```sh
od -An -tu1 -j4 -N1 /sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c
od -An -tu1 -j4 -N1 /sys/firmware/efi/efivars/SetupMode-8be4df61-93ca-11d2-aa0d-00e098032b8c
```

(The four skipped bytes are efivarfs's attribute word.) `raven-install` reads
exactly these, reports them to a front-end as `preflight.secureboot` and
`preflight.setupmode`, and prints them in the preflight block.

A firmware that claims `SecureBoot=1` *and* `SetupMode=1` is claiming something
the specification does not have. `read_secureboot_state()` believes the
enforcement bit — that is the one that decides whether the next boot works — and
drops the claim that keys can be written.

---

## Doing it from the installer

The installer offers three answers, as `--secure-boot MODE`, as the answers-file
key `secureboot`, and as a combo box in the Secure Boot group on the graphical
installer's disk page.

| Mode | What happens | Reversible by |
|---|---|---|
| `skip` *(default)* | Nothing. The firmware is not touched. | — |
| `sign` | A key set is created in the new system and the three binaries are signed. **No key is enrolled.** Firmware that is not checking ignores a signature entirely, so nothing about the next boot changes. | `rm -rf /var/lib/sbctl`, or ignoring it |
| `enroll` | All of the above, and then the key set is enrolled as the machine's Platform Key — Microsoft's certificates alongside it. Only in setup mode. | `sbctl reset`, or the firmware's *restore factory keys* |

The middle one is the whole safety argument, and it is why this is three choices
rather than a switch. `sign` lets somebody sign now, confirm the machine still
boots, and enrol afterwards with one command. Nothing is staked on getting it
right first time.

**`enroll` is refused, not downgraded, when the firmware is not in setup mode.**
It stops before the first write, with an explanation. Quietly falling back to
`sign` would leave somebody believing their machine is ready for Secure Boot to
be switched on when the firmware has never heard of their key — and they would
find that out at a boot that does not happen.

### The order of operations, and why it is that order

Everything reversible happens before the one step that touches the firmware:

1. `sbctl create-keys` — writes `/var/lib/sbctl` **on the new disk**.
2. `sbctl sign -s <file>` for each of the three binaries. The `-s` matters: it
   records the file in sbctl's own database, which is what makes `sbctl
   sign-all` re-sign the right files after a kernel update.
3. The firmware state is read **again**, because minutes have passed.
4. `sbctl enroll-keys --microsoft`, and only if every signature above succeeded.

If any signature failed, nothing is enrolled and the run says so. A firmware
that trusts this machine's key plus a boot chain that is only partly signed is a
machine that stops at the loader.

### Why it runs inside the new system

sbctl keeps its key set, and its list of files to re-sign, in `/var/lib/sbctl`
on the machine it was run on. The live image's copy of that is a tmpfs that
stops existing at the next reboot — so keys made there would be enrolled into
the firmware and then thrown away, leaving a machine whose firmware trusts a key
nobody has and whose next kernel update cannot be signed by anyone.

So `setup_secure_boot()` chroots into the target and runs the target's sbctl.
That also means the step runs **after** the package profile is installed rather
than with the bootloader, because sbctl arrives with the desktop and developer
profiles and does not exist a moment earlier.

### Why Microsoft's certificates go in too

`--microsoft` is not a concession, it is what keeps the machine working:

- A dual-booting machine's Windows loader is signed by exactly those
  certificates, and RavenBoot chainloads it through the firmware's `LoadImage`,
  which checks. Without them, Windows stops booting.
- Option ROMs on plug-in cards are signed by them too. On some machines that is
  the discrete GPU; on others it is the network card the firmware boots from.

Somebody who genuinely wants only their own key can run `sbctl enroll-keys`
without the flag afterwards, on a machine that is already booting. That is a
decision to make deliberately, not one to make for somebody during an install.

---

## Doing it by hand

This is the same sequence the installer runs, and it is the route to take when
sbctl was not on the system at install time — which today is most of the time;
see **What has not been done**.

```sh
# 1. Check where you stand.
sbctl status

# 2. Make a key set. Lives in /var/lib/sbctl.
sudo sbctl create-keys

# 3. Sign the boot chain. -s records each file for later re-signing.
sudo sbctl sign -s /boot/efi/EFI/BOOT/BOOTX64.EFI
sudo sbctl sign -s /boot/efi/EFI/raven/raven-boot.efi
sudo sbctl sign -s /boot/efi/EFI/raven/vmlinuz
sudo sbctl verify

# 4. Only with the firmware in setup mode:
sudo sbctl enroll-keys --microsoft

# 5. Reboot ONCE with Secure Boot still off and check the machine starts.
# 6. Then turn Secure Boot on in the firmware.
```

Step 5 is not optional advice. The place this fails has no error message.

To get into setup mode: firmware setup (**F2** at power-on on ASUS; **Advanced
Mode** is **F7**), *Security* → *Secure Boot* → *Key Management* → *Clear Secure
Boot Keys*. Leave Secure Boot itself off, save, exit.

---

## Undoing it

In order of how much you want to need them.

**Give the firmware its vendor keys back.** From the running system:

```sh
sudo sbctl reset          # removes the Platform Key; back to setup mode
```

or, from the firmware setup, *Restore Factory Keys* / *Install default Secure
Boot keys* in the same Key Management menu. Either one is enough; the firmware
one works when Linux does not.

**The machine will not boot after turning Secure Boot on.** Turn Secure Boot
off again in the firmware. That is the entire recovery. Nothing enrolled here
removes the firmware's ability to stop checking, and nothing enrolled here
deletes RavenBoot, the kernel or the boot configuration — enrolment adds a key,
it does not take anything away. The disk is untouched and the machine boots as
it did before.

**The signatures.** They can be left where they are: an EFI binary carrying a
signature no firmware trusts is loaded exactly as one carrying no signature at
all. To be thorough, `rm -rf /var/lib/sbctl` and reinstall the bootloader.

**BitLocker.** If Windows shares the machine and has BitLocker on, changing the
Secure Boot state changes what the TPM measured, and Windows will ask for its
recovery key once. Have it (aka.ms/myrecoverykey), or suspend BitLocker in
Windows first. `raven-install` already warns about this for the RavenBoot menu;
it applies to enrolling keys as well, and for the same reason.

---

## After a kernel update — the one ongoing hazard

A signed `vmlinuz` stops being signed the moment it is replaced.

Nothing in RavenLinux currently copies a new kernel onto the ESP automatically —
`raven-install` puts it there and nothing else moves it — so today this is only
a hazard for somebody who updates it by hand. Whoever writes that automation
must re-sign afterwards. `sbctl sign -s` recorded the files, so it is one
command:

```sh
sudo sbctl sign-all
sudo sbctl verify
```

**Do this before rebooting.** With Secure Boot on, an unsigned kernel is a
machine that shows the RavenBoot menu, is chosen from, and stops.

If a kernel-update path is ever added, the right place for this is a
`rvn` post-transaction hook next to `configs/rvn/hooks.d/50-snapshot.toml` —
the mechanism is already there and already ships one hook.

---

## What has not been done

Read this part. It is the reason this page exists.

**Nothing here has ever run against real firmware, or against a real sbctl.**
There is no sbctl on this development machine (`rvn find sbctl` shows
`extra/sbctl 0.18-2`; it is not installed and this session may not install it),
there is no qemu with an OVMF Secure Boot build, and no key has been enrolled
into anything. Every sbctl call in `setup_secure_boot()` has been exercised
against a shim that records what it was asked to do. **The call sequence, the
ordering, the refusals and the messages are tested; the behaviour of the real
`sbctl create-keys`, `sbctl sign` and `sbctl enroll-keys` is not.** Assume a VM
install with OVMF happens before anyone points this at a laptop.

**The sbctl command line is taken from its documentation, not from the binary.**
`create-keys`, `sign -s`, `verify` and `enroll-keys --microsoft` are stable and
long-standing, and two of the places they could have drifted are handled:
`enroll-keys --microsoft` falls back to the older `-m` spelling, and both
`/var/lib/sbctl/keys` (0.14 and later) and `/usr/share/secureboot/keys` (before
it) are recognised as an existing key set. If the interface is different from
what is assumed, the failure lands on `create-keys` or `sign` — both of which
are *before* the enrolment — so the result is an install that signed nothing,
not a machine that will not start.

**sbctl is not on the live image.** It is in `configs/installer/profiles/`
`desktop.packages` and `developer.packages`, so an installed desktop or
developer system gets it; `minimal.packages` deliberately adds nothing and so
has none. It is **not** in `scripts/stages/stage2-native.sh`'s `utils` list,
which is what puts binaries in the live ISO's sysroot. That is not a problem for
the design — the work deliberately happens inside the target, not on the live
image — but it does mean that today the Secure Boot step is only reachable on a
desktop or developer install whose packages are installed *during* the install
rather than at first boot. Everywhere else the installer says so, does nothing,
and prints the manual commands above. That is the correct degradation and it is
also, today, the common case.

**No TPM, no measured boot, no unified kernel image.** There is no
`systemd-stub` equivalent here and no UKI, so the kernel command line is not
covered by any signature (see above). TPM key sealing for the LUKS work is
explicitly out of scope elsewhere in this tree and interacts with this: a
TPM-sealed key measures the boot chain, so it wants the signing story settled
first.

**Module signing is not done, on purpose.** See above. It is the largest
remaining gap in the chain and it is the one with a real cost attached.

**Nothing re-signs the kernel automatically.** See above.

---

## Where the code is

| What | Where |
|---|---|
| Reading `SecureBoot` and `SetupMode` | `scripts/installer/raven-install`, `efi_global_var_byte()` / `read_secureboot_state()` |
| What is said about each firmware state | `report_secureboot_state()` |
| The offer, in the terminal wizard | `ask_secure_boot()` |
| The one refusal that is fatal | `resolve_secure_boot()` |
| Creating keys, signing, enrolling | `setup_secure_boot()` / `secure_boot_run_sbctl()` |
| The manual commands, in one place | `secure_boot_manual_steps()` |
| What the probe reports | `preflight.secureboot`, `preflight.setupmode`, `preflight.sbctl`, `preflight.sbctl_profiles` |
| The graphical front-end | `installer-ui/src/pages.rs` (Secure Boot group, summary row), `installer-ui/src/probe.rs` (`can_enroll_keys`, `sbctl_with_profile`), `installer-ui/src/install.rs` (the notes on the last page) |
| The package | `configs/installer/profiles/desktop.packages`, `developer.packages` |
| Why modules are not signed | `configs/kernel/config-6.17-raven` at `CONFIG_MODULE_SIG`, and `docs/kernel-hardening.md` |
