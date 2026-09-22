# Recovering a machine at the RavenBoot menu

A Raven machine that will not finish booting usually fails in the initramfs, and
the initramfs reads four arguments whose entire purpose is to get past whatever
it is stuck on. This is how to give it one.

The short version: highlight the entry you want, press **`e`**, type the
argument, press **Enter**. What you typed is appended to that entry's command
line for that one boot and is written nowhere.

## The keys

| Key | What it does |
|---|---|
| Up / Down | move the selection |
| Enter | boot the selection, or open a submenu |
| Esc / Backspace | leave a submenu |
| **`e`** | open the argument prompt for the selected entry |

Inside the prompt the keyboard belongs to the prompt: printable characters are
added, Backspace deletes one, **Enter** boots with what you typed, and **Esc**
closes the prompt and changes nothing. The arrow keys do nothing while it is
open, deliberately — the selection must not move out from under a line that was
typed for the entry on screen.

`e` is GRUB's key for the same job, which is why it is that letter. It is
offered only on entries that are actually booted with a kernel command line —
every `type = linux-efi` entry, which is all of the ones `raven-install` writes
that boot a kernel: *RavenLinux*, *Desktop*, *verbose* and *rescue shell*.
Selecting *Reboot*, *Shut Down*, *UEFI Shell* or a Windows entry and pressing
`e` does nothing, because for those the field would be discarded.

The prompt is also reachable during the five-second countdown. Pressing `e`
there stops the clock and opens it in one keystroke rather than two.

## What it does and does not do

It **appends**. There is no full-line editor; what you type is added to the end
of the entry's existing command line and nothing is removed. That is not a
shortcut, it is the behaviour that recovers a machine: both parsers that read
this string — `raven_root_from_cmdline` in `scripts/build-initramfs.sh` and the
kernel's own — take the *last* setting of a key as the winner. `raven-install`
writes `resume=UUID=…` into every entry it creates, so `noresume` only beats it
by arriving afterwards. An editor that let you retype the whole line would also
let you lose `root=` at three in the morning.

It is **one boot**. The menu hands the kernel a copy of the entry.
`\EFI\raven\boot.cfg` is not reopened and not rewritten, so a boot that fails
comes straight back to a menu that has forgotten the text. To make a change
permanent, edit `boot.cfg` on the ESP from a booted system.

It takes **printable ASCII only**. `bootloader/src/linux.rs` widens the command
line to UCS-2 one byte at a time, so a character typed on a non-US keyboard
would reach the kernel as mojibake in the middle of an argument. Those keys are
refused rather than accepted and mangled.

It stops at **2048 bytes**, counting the entry's existing line. That is x86-64
Linux's `COMMAND_LINE_SIZE`, past which the kernel truncates without saying so —
and what it would drop is the tail, which is exactly what you just typed. The
prompt refuses the keystroke instead, which is the only version of this you can
see happening.

There is **no keyboard, no prompt**. When the firmware offers no text-input
protocol at all — a headless QEMU guest, most commonly — RavenBoot does not
draw a menu; it boots the default entry and says so. That path is unchanged.

## The four arguments

All four are parsed in `raven_root_from_cmdline` and `raven_try_resume` in
`scripts/build-initramfs.sh`. An empty prompt lists them, because the machine
you would look them up on is the one in front of you and it is not booting.

### `noresume`

Abandons a hibernation image. `raven-install` adds `resume=UUID=<swap>` to every
entry on a machine that has swap, and the initramfs writes it to
`/sys/power/resume` before the root is mounted. If the image is bad, or the
hardware changed under it, the restore hangs or panics — and because every entry
carries the same `resume=`, the verbose and rescue entries hang at the same
point. `noresume` clears it and the next boot is an ordinary cold one. This is
the argument this prompt exists for.

`raven-snapshot` already puts `noresume` into every snapshot entry it generates,
for a stronger version of the same reason: a hibernation image is only valid
against the filesystem as it stood when the image was written, and a snapshot is
not that filesystem.

### `rootdelay=N`

Seconds to keep looking for the root device, replacing a default of 30
(`RAVEN_ROOT_WAIT` in `build-initramfs.sh`; the bare `rootwait` sets 60). The
symptom that calls for it is a machine that gives up and drops to the rescue
shell saying it cannot find `root=` — a USB enclosure behind a hub, or an
adapter that re-enumerates, can take longer than half a minute to settle.
`rootdelay=90` before trying anything else will tell you whether the device is
slow or absent, which are two different repairs. A value that is not a whole
number is refused with a warning and the default is kept, rather than being
turned into arithmetic that fails on every pass of the poll — which would mean
no waiting at all, on the one boot where somebody was asking for some.

### `crypttries=N`

How many passphrase attempts an encrypted root gets before the initramfs gives
up and drops to the rescue shell. The default is 3, which is not many when the
keyboard layout at the prompt is not the one the passphrase was chosen on.
Validated exactly like `rootdelay=`, and `0` is refused along with the malformed
values — zero attempts at a prompt somebody is standing in front of is never
what was meant.

### `raven.live`

Boots the live image even from a disk whose command line has a `root=` on it.
The initramfs clears the root spec and takes the live path;
`init/src/overrides.rs` reads the same word to keep the rest of the system in
live mode. This is the whole word — `raven.livepatch` is not it.

## What this cannot fix

The prompt appends arguments. It cannot remove one, so a `root=` pointing at a
device that no longer exists is still a trip to another machine to edit
`boot.cfg` — although `root=` given twice takes the last one, so appending a
correct `root=UUID=…` does work as an override.

It cannot repair a kernel or initramfs that is missing from the ESP, and it
cannot help a machine whose firmware will not run RavenBoot at all. For the
second of those see [`secure-boot.md`](secure-boot.md).

## The security note

Anyone who can press `e` on the physical keyboard can add `init=/bin/sh` and get
a root shell without the disk's passphrase — unless the root is encrypted, in
which case they still have to unlock it.

That is the same exposure the menu already had, not a new one: `boot.cfg` is a
plain file on an unsigned FAT partition, and a *Rescue shell* entry carrying
`init=/bin/bash` is in the menu on every installed machine by default. Physical
access has never been part of what this boot chain defends against, and
`secure-boot.md` says so in the section on the boot configuration not being
signed. If a machine needs to survive an attacker at the keyboard, the answer is
a firmware password plus full-disk encryption, and neither of those is something
the bootloader can provide for itself.

## Where the code is

| Part | File |
|---|---|
| The prompt's rules, and their tests | `bootloader/src/cmdline.rs` |
| The key, and the two menus that draw it | `bootloader/src/main.rs` |
| What the arguments do | `scripts/build-initramfs.sh` |
| What is written into `boot.cfg` | `scripts/installer/raven-install` |
