# Giving a service a resource policy

Every service `raven-init` starts runs in a cgroup of its own, at
`/sys/fs/cgroup/raven.slice/<name>/`, and a `[[services]]` block may say how
much of the machine it is allowed to take. This is the administrator's side of
that: what the keys do, what they are set to on a stock image, how to check
that a setting took effect, and the two places where the mechanism does not
reach.

The reference lives in the shipped config itself -- `/etc/raven/init.toml`
carries a comment for every key beside the services it applies to -- because a
setting is worth finding where somebody is already looking. This file is the
longer version, and covers the parts that do not fit in a comment.

## The keys

| Key | Where it lands | Unset means |
|-----|----------------|-------------|
| `nice` | `setpriority(2)`, between fork and exec | inherit init's, which is 0 |
| `oom_score_adj` | `/proc/self/oom_score_adj`, between fork and exec | 0, the kernel's own score |
| `memory_max` | `memory.max` in the service's cgroup | no ceiling |
| `cpu_weight` | `cpu.weight` | the kernel's 100 |
| `io_weight` | `io.weight` | the kernel's 100 |
| `[services.limits]` | `setrlimit(2)`, between fork and exec | whatever init inherited |

The per-process half -- `nice`, `oom_score_adj` and the limits table -- is
applied *before* the service drops to its `user =` account. That ordering is
what makes a negative nice value, a negative OOM score and a raised hard limit
possible at all: each of the three needs privilege the account being dropped to
does not have, and init still has it at that point.

`[services.limits]` is a TOML sub-table, so it must come after every plain key
of the service block it belongs to. A `restart = true` written underneath one
becomes `limits.restart` and is quietly ignored -- the service still starts,
and it does not come back when it dies. The same is true of
`[services.demand]`.

## What the stock image sets, and why

Four services in `/etc/raven/init.toml` carry a policy. The list is short on
purpose: a number nobody has measured is worse than no number, and the shipped
config sets no `memory_max` at all for exactly that reason. A ceiling guessed
below a daemon's real working set turns something that works into something
that is killed under precisely the load it was installed to handle, and the
only evidence is a `SIGKILL` with no message attached.

**`dbus`** gets `oom_score_adj = -500` and `nofile = 8192`. It is two or three
megabytes resident, so the kernel's memory-weighted score would never pick it
anyway; the negative value is cheap insurance against the cases where the
victim is not chosen by that arithmetic. What it buys off is total, because the
portals, the settings app and most of what the desktop does are clients of that
socket and they do not die when it goes away -- they hang on it. The descriptor
limit is the other classic dbus failure: a bus holds one per connected client
and one per match rule it is watching, and meeting the kernel's soft default of
1024 shows up as `accept()` returning `EMFILE` and a bus that stops answering
without dying.

**`faced`** gets `nice = 5`, `oom_score_adj = 300` and `core = "0"`. The nice
value is about what it does at start rather than in use: it optimises two ONNX
graphs before it binds its socket -- which is why its `ready_timeout` is 30 and
not 5 -- and it spends that CPU during boot, while the compositor is drawing
its first frames. A nice value is not a cap, so on an idle machine it still
gets every core it asks for. The positive OOM score volunteers it: those two
networks make it the largest resident process started from that file, so it is
already near the top of the kernel's list, and saying so explicitly states what
its death *costs* rather than restating its size. Losing face unlock leaves the
password; the thing the kernel would otherwise reach for is the compositor, and
that loses the session. `restart = true` brings it back.

**`cawd`, `fprintd` and `faced`** are all given `core = "0"`. Each holds a
credential in memory for as long as it is answering -- saved wireless
passphrases, finger templates, face embeddings -- and a core dump is a copy of
everything a process had in memory, written to disk by the kernel with no say
from the program that owns it. There is nothing to be learned from a crash in
any of the three that is worth leaving that file in `/var`.

`init/tests/parse_shipped_config.rs` asserts all of it, so an edit that drops a
value, or that moves a sub-table above a plain key and lets TOML reparent it,
fails `cargo test` instead of failing on a laptop.

## Checking that a setting took

```sh
raven-rc status faced          # memory, CPU time and pid count from the cgroup
cat /sys/fs/cgroup/raven.slice/faced/memory.max
cat /sys/fs/cgroup/raven.slice/faced/cpu.weight
grep -E 'Max core|Max open' /proc/$(pgrep -x raven-faced)/limits
cat /proc/$(pgrep -x raven-faced)/oom_score_adj
```

`raven-rc status` reads `memory.current`, `cpu.stat` and `pids.current` back
out of the kernel rather than adding up `/proc`, so its numbers and the files
above are the same numbers.

A change to a service's resource policy takes a `raven-rc reload` followed by a
restart of that service: reload notices that the definition changed, but the
limits are applied at fork, so the running process keeps the policy it was
started with.

## Weights are not caps

`cpu_weight` and `io_weight` are relative shares. A service at 50 is **not**
limited to half a core -- it gets every idle cycle it asks for, exactly as it
would with no setting at all, and the number decides only who yields when two
services want the same core at the same moment. That is almost always what is
wanted from a supervisor: capping a daemon that is competing with nothing
wastes the machine it was capped on.

`io_weight` additionally needs a weight-capable I/O policy for the device --
BFQ, or iocost with a cost model. On a machine using `mq-deadline` the
`io.weight` file does not exist; the setting is reported once in the log and
ignored, rather than failing the start.

## Where it degrades

If cgroup v2 is not mounted at `/sys/fs/cgroup`, or the tree is not writable,
the cgroup half of the mechanism becomes a no-op with a single warning in
`init.log` -- one warning, not one per service and not one per restart. The
per-process half still applies, because `setpriority`, `setrlimit` and
`oom_score_adj` need no cgroup. A definition that uses these keys is always
still a definition that starts.

## The two services this cannot reach

`ravend` (the login daemon) and `wayland-session` (which starts huginn) are the
obvious candidates for a strongly negative `oom_score_adj`: the compositor
being chosen by the OOM killer is the worst available outcome on a desktop,
because it takes every window with it. Neither can be given one in
`/etc/raven/init.toml`, and writing a block for them there is worse than
leaving them alone.

Both are synthesized at boot by `init/src/overrides.rs`, from the kernel
command line and from which binaries are installed, and appear in no file. When
a service of that name *is* already in the config, `overrides::ensure_service`
merges the synthesized definition onto it -- and it deliberately copies only
`description`, `exec`, `args`, `restart`, `enabled`, `critical` and
`environment`, leaving the operator's `after`, `ready_path`, `runtime_dirs`,
`user`, `stop_*` and resource fields alone. That is the right rule (it is what
stops a synthesized default silently undoing a hand-written `memory_max` on
every boot), but it means a hand-written block is taken as authoritative for
the fields it *omits* too:

- a `ravend` block would lose `after = ["udev", "seatd"]`, and the `pre_exec`
  that settles udev before its compositor starts. Without the settle, the
  greeter takes simpledrm -- the EFI framebuffer -- and dies when the real DRM
  driver loads and the kernel revokes it;
- a `wayland-session` block cannot name `user` at all, because the session's
  account is resolved at boot. Getting it wrong runs the desktop as root, which
  is the thing that field was added to stop.

So the defaults for those two belong in `overrides.rs`, beside the rest of
their definition, where the synthesized `ServiceConfig` is built. That is a
one-line addition to each of the two `ensure_service` calls and it has not been
made yet.

## Related

- `/etc/raven/init.toml` -- the shipped config, with a comment per key.
- `ARCHITECTURE.md`, "Resource control" under Init System.
- `init/src/cgroup.rs` -- the slice, the per-service directory, and the
  fork/exec path.
- `init/src/config.rs` -- `ServiceConfig` and `ResourceLimits`, which are where
  the ranges and the parsing rules are defined.
