# The Raven signing key and the [raven] repository

RavenLinux builds its own userland -- the shell, the package manager, version
control, the init system, the desktop -- and `rvn build` turns each of those
into a real package. `[raven]` is where those packages are served from once a
machine is installed and the ISO it came from is no longer involved. It is the
only route by which an installed machine can be sent a fix for the package
manager it is running, which is also the reason it is worth attacking: whoever
can replace a package in that repository replaces `rvn` on every machine that
installs from it.

So it is signed, and the image is shipped able to check the signature. This
file is the procedure for both halves: making the key, keeping the private half
somewhere sensible, getting the public half into the image, and -- because this
is the part that pushes people into turning verification off -- building
unsigned packages locally without touching any of it.

**Nothing in the build generates a key, and nothing here is automated.** The
commands below are run once, by a person, on a machine they control.

## The shape of it

```
  a maintainer's machine, or CI            the image                installed machines
  ------------------------------           ---------                ------------------
  private half in gpg                      public half in
      |                                    /etc/pacman.d/gnupg/pubring.gpg
      | /etc/rvn/build.toml                     |
      v                                         |  [raven]
  rvn build   -> raven-init-*.pkg.tar.zst.sig   |  SigLevel = Required DatabaseRequired
  rvn repo-add -> raven.db + raven.db.sig  -->  +--> rvn sync / rvn update / rvn install
```

Three things have to line up, and each of them fails closed on its own:

| | Made by | Checked by |
|---|---|---|
| package signature | `rvn build --sign` | `SigLevel = Required` |
| database signature | `rvn repo-add --sign` | `SigLevel = DatabaseRequired` |
| the public key | `gpg --export` | `Keyring::load` reading `GPGDir/pubring.gpg` |

The `[raven]` section itself lives in `configs/rvn/pacman.conf`, between the
`# >>> raven repository` and `# <<< raven repository` markers. It is shipped
commented out; `RAVEN_REPO=1` or `RAVEN_REPO_SERVER=<url>` at build time is
what makes it live. See *Enabling the repository* below.

## 1. Generating the key

Once, on a machine the maintainer controls. Not in CI, not in a container the
build throws away, and not in this repository.

```sh
gpg --full-generate-key
#   kind:     (1) RSA and RSA
#   size:     4096
#   expires:  2y
#   name:     Raven Linux Packages
#   email:    packages@ravenlinux.org
#   comment:  signing key for the [raven] repository
```

RSA 4096 rather than an elliptic curve, on compatibility grounds and for no
other reason: `gpg`, `pacman-key` and the `pgp` crate that `rvn` verifies with
all handle RSA without question, and an image that cannot check its own
repository is an expensive way to find out that one of them disagreed about
Ed25519. If somebody proves ed25519 end to end against `rvn`, it is the better
key and this paragraph should change.

An expiry date is worth setting even though **rvn does not enforce one** --
`verify.rs` checks that a signature was made by a key in the ring and does not
look at expiry, revocation or ownertrust. The date is a note to humans and to
`gpg`, and a forcing function for the rotation question below.

Then the revocation certificate, before the key is ever used:

```sh
gpg --output raven-packages.revoke.asc --gen-revoke <fingerprint>
```

It goes wherever the backup of the private key goes, and not next to it on the
same disk. A revocation certificate you cannot reach is a compromise you cannot
announce.

## 2. Where the private half lives

**Not in this repository, not in any repository, not in the image, not in the
ISO, and not in `build/`.** No stage reads it, no manifest names it, and the
one piece of the build that touches signing (`rvn build`, through `gpg`) is
handed a key *name*, never key material. The safe rule is that the private half
never enters the RavenLinux tree at all, which is a stronger guarantee than an
ignore rule, and ignore rules are the thing people forget when they are in a
hurry.

Two arrangements are supported, both configured in `/etc/rvn/build.toml` on the
machine that builds packages:

**A maintainer's workstation.** The key stays in that machine's own GnuPG
keyring and `gpg-agent` unlocks it with whatever pinentry is set up:

```toml
[sign]
key = "8A1B2C3D4E5F60718293A4B5C6D7E8F901234567"
```

**A CI runner**, which has a key file dropped by the pipeline and no keyring
around it:

```toml
[sign]
key = "/etc/rvn/keys/raven-packages.asc"
```

`rvn` imports that file into a throwaway `GNUPGHOME` it creates 0700 beside the
package being signed, uses it for one signature and removes it, so signing
never modifies the keyring of whoever ran the command. Two things follow from
that and are worth knowing before it bites:

- A **passphrase-protected key file is refused**, not prompted for. `rvn` would
  have to hold the passphrase to pass it to `gpg`, and not holding it is the
  point of the arrangement. Use the key-identifier form with an agent, or give
  CI a key with no passphrase and treat the file as the secret it is.
- A build **killed mid-signature** leaves that 0700 directory, holding a copy
  of the secret key, inside the output directory. Clean it up, and prefer an
  output directory that is not shared.

## 3. Signing the packages and the database

With `[sign] key` configured, signing is the default: writing a key down is
saying that packages from this machine are signed, and `rvn build` does not ask
again. `--no-sign` turns it off for one run.

`scripts/stages/stage-raven.sh` deliberately does **not** pass `--no-sign`, so
an ISO build on a host that has `/etc/rvn/build.toml` produces a signed
`build/packages/raven-repo`, and one on a host without it produces an unsigned
one and says so. Note the consequence for unattended builds: a key that needs a
passphrase with no agent running will make `gpg` prompt, and an ISO build will
sit there until somebody notices.

**Order matters.** Sign the packages *before* building the database. `rvn`
only fetches a package's `.sig` when the database record says one exists, so a
`raven.db` built before the signatures were made turns every signed package in
the repository into an unsigned one -- and under `SigLevel = Required` the
repository then refuses to install anything, reporting a missing signature that
is sitting right next to the archive on the server. `rvn build --repo <dir>`
does both in the right order; `rvn repo-add <dir>` after the fact rebuilds the
database from what is on disk, which is also correct.

Checking the result, by hand:

```sh
gpg --verify build/packages/raven-repo/raven.db.tar.gz.sig \
             build/packages/raven-repo/raven.db.tar.gz
ls -l build/packages/raven-repo/*.sig
```

**Publishing.** `raven.db`, `raven.files` and `raven.db.sig` are *symlinks* to
the `.tar.gz` files beside them, which is `repo-add`'s layout. Copy the
directory with `rsync -L`, or configure the web server to follow symlinks;
otherwise the published repository has a database that cannot be fetched.

## 4. Getting the public half into the image

Export it -- binary, which is what `gpg --export` writes without `--armor`:

```sh
gpg --export --output configs/rvn/raven-signing-key.gpg <fingerprint>
```

The build looks for, in order: `RAVEN_SIGNING_PUBKEY`,
`configs/rvn/raven-signing-key.gpg`, `configs/rvn/raven-signing-key.asc`. The
armoured `.asc` form is accepted and decoded with `gpg` on the build host.
Neither default file is in this repository; committing the *public* half is a
reasonable thing to do and is a decision for whoever owns the key, not
something this document does on their behalf.

`install_raven_key` in `stage-raven.sh` then appends the key to the staged
`/etc/pacman.d/gnupg/pubring.gpg`. Three details of that are worth writing
down:

- **It appends to `pubring.gpg` rather than running `gpg --import`.** `rvn`
  reads exactly one file for its keyring (`verify.rs`, `Keyring::load`) and it
  is that one. An import on any host with a current GnuPG lands in
  `pubring.kbx`, the modern keybox, which `rvn` never opens -- the key would be
  "installed" and invisible. Appending public key packets is the whole of what
  `pubring.gpg` is; `pacman-key --list-keys` on the installed machine reads the
  result too.
- **A secret key is refused.** The first packet of an export says which half it
  is, and a tag-5 packet stops the build rather than publishing a private key
  to every machine that installs the image.
- **It is a trust decision, not a hint.** `rvn` does not consult `trustdb` or
  ownertrust: any key in that file is trusted to sign any package from any
  repository the machine is configured for. Adding one to an image is saying
  the holder of it may replace anything on the machine.

Check what an image ended up trusting:

```sh
gpg --with-colons --show-keys build/sysroot/etc/pacman.d/gnupg/pubring.gpg \
  | awk -F: '$1 == "uid" { print $10 }' | grep -i raven
```

### Why a missing key fails the build

`install_rvn_keyring` warns and continues when the build host has no Arch
keyring. That is right for *that* case: the image is complete, it simply cannot
reach Arch's mirrors until somebody populates a keyring, and the fix is
available on the installed machine.

The Raven key is the opposite case, and `install_raven_key` therefore **fails
the build** when `[raven]` is enabled and no key is available. If it does not
ship, nothing on the installed machine can repair it: the repository the key
would verify is the one that would have delivered the fix, and under
`SigLevel = Required` it declines to serve anything it cannot check. The
failure belongs to the person running the build, who can fix it in a minute,
rather than to a stranger's first `rvn update` some months later.

## 5. Enabling the repository

```sh
RAVEN_REPO=1 ./scripts/build.sh raven                    # the Server in configs/rvn
RAVEN_REPO_SERVER=https://packages.example/\$repo/os/\$arch ./scripts/build.sh raven
RAVEN_REPO_SERVER=file:///var/cache/raven/repo ./scripts/build.sh raven
```

`rvn` reads `file://` URLs directly, so a directory `rvn repo-add` has written
is a usable repository with no web server in front of it -- useful for a
machine that keeps its own packages, and the only form available while nothing
is published. The scheme changes nothing about `SigLevel`: a local repository
still has to be signed to be installed from under this policy.

Two build-time behaviours that are easy to be surprised by:

- `RAVEN_PACMAN_FROM_HOST=1` replaces the staged `pacman.conf` with the build
  host's. The `[raven]` block is **appended** to that file rather than lost
  with the one it was written in. A host configuration that already defines
  `[raven]` is left exactly as it is, on the grounds that a RavenLinux build
  host has already answered this question with a server it can reach.
- Section order is precedence. `rvn` resolves an exact package name to the
  earliest configured repository that has it, not to the highest version.
  `[raven]` is last in the shipped file, so a name that also exists in `[core]`
  or `[extra]` resolves to Arch's copy. That is safe while the repository holds
  only names nothing else has; the day it publishes RavenLinux's own `bash`,
  the block has to move above `[core]`.

Note that none of this affects how the ISO itself is built. `stage-raven.sh`
installs the packages it just built by extracting them into the sysroot and
writing the local database record; no signature is checked at image-build time,
because the archive being installed was produced ten seconds earlier by the
same script. `[raven]` is about the machine afterwards.

## 6. Building unsigned packages locally

This is the section that exists so that nobody reaches for the blunt
instrument. **Do not** relax `SigLevel` in `[options]`:

```conf
[options]
SigLevel = Optional     # NO -- this is every repository on the machine
```

That line turns off signature enforcement for `core`, `extra` and `multilib`
as well, which is to say for every package the machine will ever install, in
order to install one package you compiled yourself five minutes ago. `SigLevel`
is per-repository for exactly this reason.

### The pattern: a second repository, relaxed on its own

```conf
# /etc/pacman.conf, on a development machine only
[raven-local]
SigLevel = Optional
Server = file:///home/you/raven/build/packages/raven-repo
```

Put it **above** `[raven]` so that a locally built package wins the name, and
leave every other section alone. Then:

```sh
rvn build packages/raven/crow --srcdir <checkout> \
          --outdir build/packages/raven-repo --repo raven-local --no-sign
sudo rvn install crow
```

`Optional` still verifies a signature that is present and still checks the
SHA-256 from the database against the archive; what it stops doing is insisting
that a signature exists. The relaxation is scoped to one repository, on one
machine, holding only packages you built.

`rvn install` takes package names, not paths -- there is no
`rvn install ./crow-0.1.0-1-x86_64.pkg.tar.zst` -- which is why a local
repository is the mechanism rather than a convenience.

### The alternative: sign with your own key and keep Required

If you would rather not have a `SigLevel = Optional` section on the machine at
all, sign with a personal key and tell the machine about its public half:

```sh
rvn build packages/raven/crow --srcdir <checkout> --key you@example.com \
          --outdir ~/raven-repo --repo raven-local
gpg --export you@example.com | sudo tee -a /etc/pacman.d/gnupg/pubring.gpg >/dev/null
```

Remember what that second line means, from *§4*: that key can now sign anything
for this machine. It is a reasonable thing to do to your own workstation and
not something to do to somebody else's.

### Building the ISO without any of this

Nothing. `[raven]` is off in a default build, no key is needed, and the image
is exactly what it was before this document existed. `RAVEN_SKIP_PACKAGING=1`
goes further and installs the built binaries as loose files, which is the
behaviour that predates `rvn build` entirely.

## 7. Rotation, and the gap this leaves

The public key is baked into the image's `pubring.gpg`. There is no
`raven-keyring` package, so there is currently **no way to give an installed
machine a new key except by hand** -- which is the one thing a signed
repository is least able to deliver, since a machine that cannot verify the new
packages cannot install the package that would let it. Changing the key today
means:

- a new ISO for new installations, and
- `gpg --export <new> | sudo tee -a /etc/pacman.d/gnupg/pubring.gpg` on every
  installed machine, by whoever administers it.

Rotating signing subkeys does not avoid this: the image holds the key as it was
exported, so a subkey created afterwards is not in it either.

The fix, when somebody gets to it, is a `raven-keyring` package that owns
`/etc/pacman.d/gnupg/pubring.gpg` (or a fragment directory feeding it) and is
itself signed by the *outgoing* key, which is how Arch does it with
`archlinux-keyring`. Until that exists, treat the key as long-lived, keep the
revocation certificate somewhere you can actually reach, and do not enable
`[raven]` on images you cannot re-cut.

## 8. What it looks like when it is wrong

| What you see | What happened | What to do |
|---|---|---|
| Build stops: `[raven] is enabled ... and there is no Raven key to ship` | `RAVEN_REPO=1` with no public key on the build host | export the key, or leave `[raven]` off (§4) |
| Build stops: `... is a SECRET key, not a public one` | `gpg --export-secret-keys` instead of `gpg --export` | export the public half |
| `raven: no database signature is published, but the repository is configured as DatabaseRequired` | the database was built without a key | `rvn repo-add <dir> --name raven` on a host with `[sign] key` (§3) |
| `required signature is missing` for a package | the database was built before the packages were signed, so it records no `%PGPSIG%` | sign, then rebuild the database (§3) |
| `signed by unknown key <id>` | the image has a different key from the one that signed | check §4's `--show-keys` output against the signing fingerprint |
| `the pacman keyring could not be read` | no `pubring.gpg` in `GPGDir` at all | the build host had no keyring when the image was made; see `install_rvn_keyring` |
| `rvn sync` warns about `raven` every run and everything else works | `[raven]` points somewhere that serves nothing | set `RAVEN_REPO_SERVER`, or rebuild with `[raven]` off |

A note on that last row: one failing repository is a warning, not a fatal
error, as long as another one synced. That is why an image shipped with
`[raven]` pointing at a server that does not exist yet is an irritation rather
than a brick -- and why it is still not the default.

## Where the pieces are

| | |
|---|---|
| the `[raven]` stanza, and why each line of it says what it says | `configs/rvn/pacman.conf` |
| enabling it, and the key check | `install_raven_repo_section`, `install_raven_key` in `scripts/stages/stage-raven.sh` |
| building and signing packages | `rvn build`, `rvn repo-add`; `RavenPackageManager/src/sign.rs` |
| the signing configuration file | `RavenPackageManager/etc/rvn/build.toml` (a reference copy; nothing installs it yet) |
| what verification actually does | `RavenPackageManager/src/verify.rs`, `src/ops/sync.rs` |
