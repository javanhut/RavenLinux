//! `raven-firewall` -- the front end for the packet filter nobody wants to
//! learn nft to read.
//!
//! # Why this exists
//!
//! The policy itself is already written down, at length, in
//! `/etc/nftables.conf`, and the `nftables` one-shot in `/etc/raven/init.toml`
//! hands it to the kernel once at boot. That file is the authority and this
//! program does not replace it. What it replaces is the three things a person
//! actually does to a firewall between one reboot and the next: ask whether it
//! is on, open a port, and put the file back into force after editing it.
//!
//! Without a front end each of those is a different nft incantation with a
//! different failure mode, and the honest consequence is the one this tree has
//! seen before with other subsystems: a firewall that can only be inspected by
//! someone who already knows nft is a firewall that gets turned off the first
//! time it is suspected of something. So: three verbs, and text a person can
//! read.
//!
//! ```text
//! raven-firewall status              is it loaded, and what does it let in
//! raven-firewall allow 22            open a port, persistently
//! raven-firewall allow 5353/udp      ... udp, when tcp is not what is meant
//! raven-firewall reload              re-read /etc/nftables.conf
//! ```
//!
//! # Why there is no daemon behind it
//!
//! raven-powerd and raven-timed are binaries in this crate that front a daemon
//! of their own: with arguments they are a client of a socket, with none they
//! are the thing listening on it. This one is deliberately not that shape,
//! because there is nothing to listen. A loaded nftables ruleset lives in the
//! kernel, not in a process -- which is the same fact that makes the boot-time
//! service a `oneshot` whose success looks like `exited`. A daemon here would
//! hold no state that the kernel and one file do not already hold between
//! them, so this is a plain CLI in the shape of `raven-ports`.
//!
//! # How `allow` persists
//!
//! By editing `/etc/nftables.conf` in place, inside a pair of marker comments,
//! and touching not one byte outside them. That is the same bargain
//! `raven-timed sync off` makes with `/etc/raven/time.toml` through
//! `toml_edit` -- the file stays the hand-written, hand-commented thing its
//! author left, and a tool may still change one fact in it. nft's syntax is
//! not TOML and there is no format-preserving parser for it in this tree, so
//! the marker block is how the same promise gets kept: everything this program
//! writes is between two lines that say so, and a person may edit, reorder or
//! delete any of it by hand without confusing the next `allow`.
//!
//! The alternative considered and rejected was a generated include file, which
//! keeps `/etc/nftables.conf` byte-identical to the copy in the source tree
//! but splits the answer to "what is allowed here?" across two files, one of
//! which carries a banner telling you not to edit it. For a laptop firewall
//! that is the worse trade: the file people are told to read should be the
//! file that is true.
//!
//! # What it deliberately does not do
//!
//! It does not reorder rules, delete them, or offer a `deny` verb. The input
//! chain's policy is already `drop`, so "deny a port" means finding the rule
//! that allows it and removing it -- and a tool that removes rules it did not
//! write from a file it did not write is a tool that will eventually remove
//! the wrong one. Deleting a line from the marker block by hand is one editor
//! command and leaves the intent visible in a diff.
//!
//! Nor does it filter egress, know anything about applications, or manage
//! tables other than `inet filter`. The reasoning for the first two is in the
//! header of `/etc/nftables.conf` and belongs there, not here.
//!
//! # When nftables is not installed
//!
//! Which, as this is written, is the normal case: there is no `nft` binary in
//! the image and the shipped kernel config says `# CONFIG_NF_TABLES is not
//! set`, so the boot-time service ships disabled. Every verb here is built to
//! say that plainly rather than to fail the way a missing binary usually fails
//! -- `status` reports what the config file would allow and says nothing is
//! enforcing it, and `allow` still writes the rule down, because a port opened
//! today should still be open on the boot after the package lands.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The ruleset. The same path the `nftables` service in `/etc/raven/init.toml`
/// passes to `nft -f`, and the only file this program writes.
const NFTABLES_CONF: &str = "/etc/nftables.conf";

/// The table the shipped policy lives in, and the only one this program reads
/// or adds to. A ruleset may hold others -- anything that sets up a container
/// bridge installs its own, at its own priority -- and none of them are this
/// program's business.
const TABLE: &str = "inet filter";

/// Where `allow` writes, and the promise it makes. Everything between these
/// two lines is this program's; everything outside them is the author's and is
/// copied through untouched.
const BEGIN_MARKER: &str = "# BEGIN raven-firewall";
const END_MARKER: &str = "# END raven-firewall";

/// Where `nft` might be. Checked in this order before `$PATH`, because the
/// answer has to be good enough to put in a message: "there is no nft on this
/// machine" is a useful thing to be told, and "No such file or directory (os
/// error 2)" from a failed spawn is not.
///
/// `/usr/sbin` and `/sbin` are symlinks to `/usr/bin` and `/bin` here, so on
/// this machine several of these resolve to the same file. They are all listed
/// anyway: the split is real on other systems this crate is built on, and a
/// stat of a path that does not exist costs nothing.
const NFT_CANDIDATES: [&str; 4] = ["/usr/bin/nft", "/usr/sbin/nft", "/bin/nft", "/sbin/nft"];

// ---------------------------------------------------------------------------
// Ports
// ---------------------------------------------------------------------------

/// The two transport protocols a port number means anything for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proto {
    Tcp,
    Udp,
}

impl Proto {
    /// The token nft uses, which is also the token the user types.
    fn keyword(self) -> &'static str {
        match self {
            Proto::Tcp => "tcp",
            Proto::Udp => "udp",
        }
    }
}

/// One port to open: what `allow` takes and what a rule line says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PortSpec {
    proto: Proto,
    port: u16,
}

impl PortSpec {
    /// The nft rule that opens this port, without indentation.
    ///
    /// Unconditional on purpose: no interface, no source address, no state.
    /// Anything narrower is a decision about *this* network, and a firewall
    /// rule written once by a tool and read a year later by a person should
    /// not have a qualifier in it that nobody remembers asking for.
    fn rule(self) -> String {
        format!("{} dport {} accept", self.proto.keyword(), self.port)
    }
}

/// Parse `22`, `22/tcp` or `5353/udp`.
///
/// Numbers only. Resolving `ssh` through `/etc/services` was considered and
/// left out: the name lookup would succeed on the machine writing the rule and
/// the rule would record the number anyway, so all the alias buys is a second
/// way to spell the same thing and a new failure when `/etc/services` is thin.
fn parse_port_spec(text: &str) -> Result<PortSpec, String> {
    let (number, proto) = match text.split_once('/') {
        None => (text, Proto::Tcp),
        Some((number, "tcp")) => (number, Proto::Tcp),
        Some((number, "udp")) => (number, Proto::Udp),
        Some((_, other)) => {
            return Err(format!(
                "`{other}` is not a protocol; write tcp or udp, as in 5353/udp"
            ))
        }
    };

    let port: u16 = number
        .parse()
        .map_err(|_| format!("`{number}` is not a port number between 1 and 65535"))?;

    // Port 0 parses as a u16 and is not a port; nft would take the rule and
    // the rule would never match anything.
    if port == 0 {
        return Err("port 0 is not a port".to_string());
    }

    Ok(PortSpec { proto, port })
}

// ---------------------------------------------------------------------------
// Reading nft syntax
// ---------------------------------------------------------------------------

/// The text of one line with any trailing comment removed.
///
/// Used everywhere a line is examined for structure, so that a `{` or a rule
/// written inside a comment -- and this ruleset's comments contain several,
/// including the two ready-to-uncomment rules for ssh and CUPS -- is read as
/// prose rather than as syntax.
fn code_of(line: &str) -> &str {
    match line.find('#') {
        Some(at) => &line[..at],
        None => line,
    }
}

/// Where the `input` chain begins and ends, as line indices into `lines`.
///
/// The end is the line holding the chain's own closing brace, found by
/// counting braces rather than by looking for a `}` at the right indentation:
/// the ICMP and ICMPv6 rules in the shipped file hold multi-line set literals
/// whose braces would otherwise be mistaken for the end of the chain.
fn input_chain_span(lines: &[&str]) -> Option<(usize, usize)> {
    let start = lines.iter().position(|line| {
        let code = code_of(line).trim();
        code.starts_with("chain input") && code.ends_with('{')
    })?;

    let mut depth = 1usize;
    for (offset, line) in lines.iter().enumerate().skip(start + 1) {
        let code = code_of(line);
        depth += code.matches('{').count();
        depth = depth.saturating_sub(code.matches('}').count());
        if depth == 0 {
            return Some((start, offset));
        }
    }

    None
}

/// The input chain's policy word -- `drop` or `accept` -- from either the
/// config file or `nft list table`, which spell the hook line the same way.
fn input_policy(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let (start, end) = input_chain_span(&lines)?;
    lines[start..=end].iter().find_map(|line| {
        let (_, after) = code_of(line).split_once("policy")?;
        Some(after.trim().trim_end_matches(';').trim().to_string())
    })
}

/// The input chain's statements, one per entry, comments dropped and
/// whitespace normalised.
///
/// A statement is not a line. The shipped ruleset writes its ICMP and ICMPv6
/// rules with a set literal spread over a dozen lines, and reading those
/// line-by-line yields entries like `} accept`, which is not a rule and not
/// anything a person can act on. So lines are joined until the braces balance
/// again, which turns each of those back into the single rule it is.
fn input_statements(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let Some((start, end)) = input_chain_span(&lines) else {
        return Vec::new();
    };

    let mut statements = Vec::new();
    let mut pending = String::new();
    let mut depth = 0usize;

    for line in &lines[start + 1..end] {
        let code = code_of(line);
        let piece = normalise(code);
        if piece.is_empty() {
            continue;
        }
        if !pending.is_empty() {
            pending.push(' ');
        }
        pending.push_str(&piece);

        depth += code.matches('{').count();
        depth = depth.saturating_sub(code.matches('}').count());
        if depth == 0 {
            statements.push(std::mem::take(&mut pending));
        }
    }

    // A file whose braces never balance again is a file nft would refuse; keep
    // whatever was accumulated rather than silently losing the tail, so that
    // `status` on a half-edited ruleset still shows the half that parsed.
    if !pending.is_empty() {
        statements.push(pending);
    }

    statements
}

/// Every rule in the input chain that ends in `accept`, in the order the
/// kernel evaluates them.
///
/// Printed verbatim rather than translated into English, and that is the
/// point: a rule like `ip ttl 255 udp sport 5353 udp dport 5353 accept` says
/// exactly what it does, and any summary of it this program invented would be
/// a summary a reader then has to distrust. The order is worth keeping because
/// in a chain it is the semantics.
fn accept_rules(text: &str) -> Vec<String> {
    input_statements(text)
        .into_iter()
        .filter(|rule| rule.ends_with("accept"))
        .collect()
}

/// Collapse runs of whitespace so that a rule read out of a tab-indented
/// config file compares equal to the same rule printed by nft.
fn normalise(code: &str) -> String {
    code.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Writing nft syntax
// ---------------------------------------------------------------------------

/// What happened when a port was offered to the config file.
#[derive(Debug, PartialEq, Eq)]
enum AllowOutcome {
    /// The rule was added; here is the whole file to write back.
    Added(String),
    /// The rule is already in this program's block. Nothing to do, and not an
    /// error: `allow 22` twice should be as uneventful as it sounds.
    AlreadyManaged,
    /// A rule the author wrote by hand already opens this port, and here it
    /// is. Reported rather than duplicated, because two rules that do the same
    /// thing are how a ruleset stops being readable.
    AlreadyByHand(String),
}

/// Add `spec` to the input chain of `text`, or explain why it is already there.
///
/// Pure text in, text out, so the interesting half is testable without a
/// filesystem -- the same split `control::dispatch` makes for the same reason.
fn allow_in(text: &str, spec: PortSpec) -> Result<AllowOutcome, String> {
    let lines: Vec<&str> = text.lines().collect();
    let (start, end) = input_chain_span(&lines).ok_or_else(|| {
        format!("{NFTABLES_CONF} has no `chain input` for the rule to go in; edit it by hand")
    })?;

    let wanted = spec.rule();
    let block = managed_block_span(&lines[start..=end]).map(|(b, e)| (b + start, e + start));

    // A rule that already exists, wherever it is, means there is nothing to
    // do. The match is deliberately exact: a conditional rule such as
    // `udp sport 67 udp dport 68 accept` mentions a destination port without
    // opening it unconditionally, and claiming otherwise would be a tool
    // telling a person a port is open when it is not.
    for (index, line) in lines.iter().enumerate().take(end).skip(start + 1) {
        if normalise(code_of(line)) != wanted {
            continue;
        }
        return Ok(match block {
            Some((first, last)) if index > first && index < last => AllowOutcome::AlreadyManaged,
            _ => AllowOutcome::AlreadyByHand(wanted),
        });
    }

    let indent = rule_indent(&lines, start, end);
    let mut out: Vec<String> = lines.iter().map(|line| (*line).to_string()).collect();

    match block {
        // Slot the rule in above the END marker, so the block stays in the
        // order the ports were opened in.
        Some((_, last)) => out.insert(last, format!("{indent}{wanted}")),
        // No block yet: build one immediately before the chain's closing
        // brace. Last in the chain is the right place for an unconditional
        // accept -- `policy drop` is the chain's policy, not a rule, so
        // nothing this lands after can shadow it.
        None => {
            let mut fresh = vec![String::new()];
            for line in managed_block(&indent, &wanted) {
                fresh.push(line);
            }
            out.splice(end..end, fresh);
        }
    }

    let mut joined = out.join("\n");
    // `lines()` drops the final newline; a config file that loses it on every
    // edit is a config file whose diffs all have a spurious last hunk.
    if text.ends_with('\n') {
        joined.push('\n');
    }
    Ok(AllowOutcome::Added(joined))
}

/// The block this program owns, as indices into the slice it was given.
fn managed_block_span(lines: &[&str]) -> Option<(usize, usize)> {
    let begin = lines
        .iter()
        .position(|line| line.trim().starts_with(BEGIN_MARKER))?;
    let end = lines
        .iter()
        .skip(begin)
        .position(|line| line.trim().starts_with(END_MARKER))?;
    Some((begin, begin + end))
}

/// A fresh marker block holding one rule.
fn managed_block(indent: &str, rule: &str) -> Vec<String> {
    vec![
        format!("{indent}{BEGIN_MARKER} -- ports opened with `raven-firewall allow`."),
        format!("{indent}# Everything between these two markers is rewritten by that command;"),
        format!("{indent}# everything outside them is left exactly as you wrote it. Deleting a"),
        format!("{indent}# line here by hand is how a port gets closed again."),
        format!("{indent}{rule}"),
        format!("{indent}{END_MARKER}"),
    ]
}

/// The indentation a rule in this chain is written with.
///
/// Taken from the first rule already in the chain rather than assumed, because
/// the shipped file is indented with tabs and a file somebody reformatted with
/// spaces should not end up with both.
fn rule_indent(lines: &[&str], start: usize, end: usize) -> String {
    for line in &lines[start + 1..end] {
        if line.trim().is_empty() {
            continue;
        }
        return line[..line.len() - line.trim_start().len()].to_string();
    }
    // An empty chain: one level in from wherever its closing brace sits.
    let closing = lines[end];
    let outer = &closing[..closing.len() - closing.trim_start().len()];
    format!("{outer}\t")
}

// ---------------------------------------------------------------------------
// Talking to nft
// ---------------------------------------------------------------------------

/// Find `nft`, or say there is none.
fn nft_path() -> Option<PathBuf> {
    for candidate in NFT_CANDIDATES {
        let path = Path::new(candidate);
        if path.exists() {
            return Some(path.to_path_buf());
        }
    }

    // A machine that installed nftables somewhere unusual, or a developer
    // running this out of a build tree with a PATH of their own.
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join("nft"))
        .find(|candidate| candidate.exists())
}

/// One nft run, with both streams captured.
struct NftRun {
    ok: bool,
    stdout: String,
    stderr: String,
}

fn run_nft(nft: &Path, args: &[&str]) -> io::Result<NftRun> {
    let out = Command::new(nft).args(args).output()?;
    Ok(NftRun {
        ok: out.status.success(),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    })
}

/// The one message every verb gives when there is no packet filter to talk to.
///
/// It names the package, because "nftables is not installed" without the next
/// step is the kind of true statement that still leaves someone searching.
fn explain_missing_nft() {
    eprintln!("nftables is not installed on this machine: no nft in /usr/bin, /usr/sbin or $PATH.");
    eprintln!("  Install it with:  sudo rvn install nftables");
    eprintln!("  The kernel also needs CONFIG_NF_TABLES, which the shipped config does not set yet.");
}

/// nft needs CAP_NET_ADMIN for everything interesting, and says so in a way
/// worth translating once rather than pasting through three times.
fn looks_like_permission_trouble(stderr: &str) -> bool {
    stderr.contains("Operation not permitted") || stderr.contains("Permission denied")
}

// ---------------------------------------------------------------------------
// Verbs
// ---------------------------------------------------------------------------

/// `raven-firewall status` -- is anything filtering, and what does it let in.
fn do_status() -> i32 {
    let config = std::fs::read_to_string(NFTABLES_CONF);

    let Some(nft) = nft_path() else {
        println!("firewall     not enforced -- nftables is not installed");
        // The file is still worth printing. It is what the machine will do the
        // first time it boots with nft present and the service enabled, and
        // somebody editing it today deserves to see their edit parsed.
        match &config {
            Ok(text) => {
                println!("ruleset      {NFTABLES_CONF} (not loaded)");
                print_policy_and_rules(text);
            }
            Err(e) => println!("ruleset      cannot read {NFTABLES_CONF}: {e}"),
        }
        println!();
        explain_missing_nft();
        return 0;
    };

    let listing = match run_nft(&nft, &["list", "table", TABLE]) {
        Ok(run) => run,
        Err(e) => {
            eprintln!("cannot run {}: {e}", nft.display());
            return 1;
        }
    };

    if !listing.ok {
        if looks_like_permission_trouble(&listing.stderr) {
            eprintln!("cannot read the ruleset: nft needs root. Try `sudo raven-firewall status`.");
            return 1;
        }
        // nft exits non-zero for an absent table, which is the ordinary state
        // of a machine whose firewall service has never run. Not an error.
        println!("firewall     not loaded -- no `{TABLE}` table in the kernel");
        println!("  Load it now with:     raven-rc start nftables");
        println!("  At every boot with:   raven-rc enable nftables");
        if let Ok(text) = &config {
            println!();
            println!("What {NFTABLES_CONF} would load:");
            print_policy_and_rules(text);
        }
        return 0;
    }

    println!("firewall     loaded ({TABLE})");
    println!("ruleset      {NFTABLES_CONF}");
    print_policy_and_rules(&listing.stdout);

    // Only worth saying when the two can actually differ, which is once a
    // ruleset is loaded: a file edited since the last `nft -f` is a firewall
    // whose config no longer describes it, and the symptom is a rule that is
    // in the file and does nothing.
    if let Ok(text) = &config {
        if accept_rules(text) != accept_rules(&listing.stdout) {
            println!();
            println!("note: the loaded ruleset and {NFTABLES_CONF} disagree.");
            println!("      `raven-firewall reload` makes the file the one in force.");
        }
    }

    0
}

/// The two lines and the list that both halves of `status` print.
fn print_policy_and_rules(text: &str) {
    match input_policy(text) {
        Some(policy) => println!("inbound      {policy} unless a rule below allows it"),
        None => println!("inbound      (no input chain found)"),
    }

    let rules = accept_rules(text);
    if rules.is_empty() {
        println!("allowed      nothing -- every inbound packet is dropped");
        return;
    }

    println!("allowed      {} rules, in the order they match:", rules.len());
    for rule in rules {
        println!("  {rule}");
    }
}

/// `raven-firewall allow <port>[/tcp|udp]`.
fn do_allow(spec: PortSpec) -> i32 {
    let text = match std::fs::read_to_string(NFTABLES_CONF) {
        Ok(text) => text,
        Err(e) => {
            eprintln!("cannot read {NFTABLES_CONF}: {e}");
            return 1;
        }
    };

    match allow_in(&text, spec) {
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
        Ok(AllowOutcome::AlreadyManaged) => {
            println!("{} is already allowed in {NFTABLES_CONF}.", spec.rule());
        }
        Ok(AllowOutcome::AlreadyByHand(rule)) => {
            println!("already allowed by a rule somebody wrote by hand:");
            println!("  {rule}");
            println!("Left alone, so that the rule keeps its place and its comment.");
        }
        Ok(AllowOutcome::Added(updated)) => {
            if let Err(e) = write_config(&updated) {
                eprintln!("cannot write {NFTABLES_CONF}: {e}");
                if e.kind() == io::ErrorKind::PermissionDenied {
                    eprintln!("  Opening a port is a root-only edit. Try `sudo raven-firewall allow ...`.");
                }
                return 1;
            }
            println!("added to {NFTABLES_CONF}:  {}", spec.rule());
        }
    }

    apply_live(spec)
}

/// Put the rule into the running kernel too, if there is one to put it into.
///
/// A targeted `add rule` rather than a reload, and the difference matters:
/// `/etc/nftables.conf` begins with `flush ruleset`, so re-reading it takes
/// every chain out of its hook and puts it back. For the length of that
/// transaction there is no input chain, and no input chain means no
/// `policy drop` -- a machine briefly accepting everything is a strange price
/// to pay for opening one port. Appending one rule to a live chain has no such
/// window, and the chain's policy is not a rule, so nothing can shadow it.
fn apply_live(spec: PortSpec) -> i32 {
    let Some(nft) = nft_path() else {
        println!("Nothing is enforcing it yet: nftables is not installed.");
        println!("The rule is in the file and will be loaded once it is.");
        return 0;
    };

    let listing = match run_nft(&nft, &["list", "table", TABLE]) {
        Ok(run) => run,
        Err(e) => {
            eprintln!("saved, but cannot run {}: {e}", nft.display());
            return 1;
        }
    };

    if !listing.ok {
        if looks_like_permission_trouble(&listing.stderr) {
            eprintln!("saved, but nft needs root to load it. Run `sudo raven-firewall reload`.");
            return 1;
        }
        println!("The ruleset is not loaded, so nothing changed in the kernel.");
        println!("Load it with:  raven-rc start nftables");
        return 0;
    }

    if accept_rules(&listing.stdout).iter().any(|r| *r == spec.rule()) {
        println!("The running ruleset already had it.");
        return 0;
    }

    let rule = spec.rule();
    let args = ["add", "rule", "inet", "filter", "input", &rule];
    match run_nft(&nft, &args) {
        Ok(run) if run.ok => {
            println!("Added to the running ruleset as well.");
            0
        }
        Ok(run) => {
            eprintln!("saved, but the running ruleset would not take it:");
            eprintln!("{}", run.stderr.trim_end());
            eprintln!("  `raven-firewall reload` will apply the file as a whole.");
            1
        }
        Err(e) => {
            eprintln!("saved, but cannot run {}: {e}", nft.display());
            1
        }
    }
}

/// `raven-firewall reload` -- hand the file to the kernel again.
fn do_reload() -> i32 {
    let Some(nft) = nft_path() else {
        explain_missing_nft();
        return 1;
    };

    if !Path::new(NFTABLES_CONF).exists() {
        eprintln!("there is no {NFTABLES_CONF} to load");
        return 1;
    }

    // Checked first, and separately. `nft -f` on a ruleset with a syntax error
    // has already run `flush ruleset` by the time it reaches the bad line, so
    // the machine is left with no firewall at all and a message about line 47.
    // `-c` parses without committing, which turns that into a refusal.
    match run_nft(&nft, &["-c", "-f", NFTABLES_CONF]) {
        Ok(run) if !run.ok => {
            eprintln!("{NFTABLES_CONF} was not loaded; nft objected to it:");
            eprintln!("{}", run.stderr.trim_end());
            eprintln!("The ruleset in force is unchanged.");
            return 1;
        }
        Err(e) => {
            eprintln!("cannot run {}: {e}", nft.display());
            return 1;
        }
        Ok(_) => {}
    }

    match run_nft(&nft, &["-f", NFTABLES_CONF]) {
        Ok(run) if run.ok => {
            println!("loaded {NFTABLES_CONF}");
            0
        }
        Ok(run) => {
            if looks_like_permission_trouble(&run.stderr) {
                eprintln!("nft needs root to load a ruleset. Try `sudo raven-firewall reload`.");
            } else {
                eprintln!("{}", run.stderr.trim_end());
            }
            1
        }
        Err(e) => {
            eprintln!("cannot run {}: {e}", nft.display());
            1
        }
    }
}

/// Replace the config file without ever letting a half-written one exist.
///
/// Write a sibling and rename over the target: rename is atomic within a
/// filesystem, so a crash or a full disk leaves the old ruleset intact. The
/// cost of getting this wrong is a boot that loads a truncated file, and the
/// `flush ruleset` on its first line means the machine would come up with the
/// firewall flushed and nothing put back.
fn write_config(text: &str) -> io::Result<()> {
    let target = Path::new(NFTABLES_CONF);
    let temp = target.with_extension("conf.raven-firewall");

    // Carry the original's permissions across; the default for a new file
    // would be whatever the process umask says, and this one is read by a
    // service that runs before anything else on the machine.
    let mode = std::fs::metadata(target).ok().map(|m| {
        use std::os::unix::fs::PermissionsExt;
        m.permissions().mode()
    });

    std::fs::write(&temp, text)?;
    if let Some(mode) = mode {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(mode))?;
    }

    match std::fs::rename(&temp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            // A failed rename is the one step that leaves debris behind.
            let _ = std::fs::remove_file(&temp);
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (command, operands): (&str, &[String]) = match args.split_first() {
        None => ("status", &[]),
        Some((first, rest)) => (first.as_str(), rest),
    };

    let code = match command {
        "status" | "list" => do_status(),
        "allow" => match operands.first() {
            None => {
                eprintln!("raven-firewall allow: needs a port, as in `allow 22` or `allow 5353/udp`");
                2
            }
            Some(text) => match parse_port_spec(text) {
                Ok(spec) => do_allow(spec),
                Err(e) => {
                    eprintln!("raven-firewall allow: {e}");
                    2
                }
            },
        },
        "reload" => do_reload(),
        "-h" | "--help" | "help" => {
            usage();
            0
        }
        other => {
            eprintln!("raven-firewall: unknown command `{other}`");
            usage();
            2
        }
    };

    std::process::exit(code);
}

fn usage() {
    eprintln!(
        "usage: raven-firewall [status | allow PORT[/tcp|/udp] | reload]\n\n\
         status   whether the ruleset is loaded and what it lets in (default)\n\
         allow    open a port, in /etc/nftables.conf and in the running ruleset\n\
         reload   re-read /etc/nftables.conf after editing it by hand\n\n\
         Ports are numbers, and tcp unless you say otherwise: `allow 22`,\n\
         `allow 5353/udp`. To close one again, delete its line from the\n\
         raven-firewall block in /etc/nftables.conf and reload.\n\n\
         The ruleset itself, with the reasoning behind every rule in it, is\n\
         /etc/nftables.conf. It is loaded at boot by the `nftables` service;\n\
         `raven-rc status nftables` says whether that succeeded."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A chain shaped like the shipped one: tab indentation, a multi-line set
    /// literal whose braces must not be mistaken for the chain's, and a rule
    /// written inside a comment.
    const SAMPLE: &str = "\
#!/usr/bin/nft -f
# A header.

flush ruleset

table inet filter {

\t# A comment about the chain.
\tchain input {
\t\ttype filter hook input priority filter; policy drop;

\t\tct state established,related accept
\t\tiif \"lo\" accept
\t\tct state invalid drop

\t\tmeta l4proto icmp icmp type {
\t\t\techo-request,
\t\t\ttime-exceeded
\t\t} accept

\t\tudp sport 67 udp dport 68 accept

\t\t# Not enabled, deliberately:
\t\t#   tcp dport 22 accept
\t}

\tchain output {
\t\ttype filter hook output priority filter; policy accept;
\t}
}
";

    #[test]
    fn a_port_is_tcp_unless_it_says_otherwise() {
        assert_eq!(
            parse_port_spec("22"),
            Ok(PortSpec { proto: Proto::Tcp, port: 22 })
        );
        assert_eq!(
            parse_port_spec("22/tcp"),
            Ok(PortSpec { proto: Proto::Tcp, port: 22 })
        );
        assert_eq!(
            parse_port_spec("5353/udp"),
            Ok(PortSpec { proto: Proto::Udp, port: 5353 })
        );
        assert_eq!(parse_port_spec("65535").map(|s| s.port), Ok(65535));
    }

    #[test]
    fn a_port_that_is_not_one_is_refused() {
        assert!(parse_port_spec("0").is_err());
        assert!(parse_port_spec("65536").is_err());
        assert!(parse_port_spec("ssh").is_err());
        assert!(parse_port_spec("22/sctp").is_err());
        assert!(parse_port_spec("").is_err());
        assert!(parse_port_spec("-1").is_err());
    }

    #[test]
    fn the_input_chain_ends_where_it_ends() {
        let lines: Vec<&str> = SAMPLE.lines().collect();
        let (start, end) = input_chain_span(&lines).expect("the sample has an input chain");
        assert!(lines[start].contains("chain input"));
        // Not the `}` that closes the icmp type set, and not the one that
        // closes the table: the chain's own.
        assert_eq!(lines[end].trim(), "}");
        assert!(lines[end - 1].trim().starts_with("#   tcp dport 22"));
    }

    #[test]
    fn the_policy_is_read_off_the_hook_line() {
        assert_eq!(input_policy(SAMPLE).as_deref(), Some("drop"));
        assert_eq!(input_policy("table inet filter {\n}\n"), None);
    }

    #[test]
    fn only_real_accepts_are_listed() {
        let rules = accept_rules(SAMPLE);
        assert_eq!(
            rules,
            vec![
                "ct state established,related accept",
                "iif \"lo\" accept",
                // One rule, however many lines its set literal took.
                "meta l4proto icmp icmp type { echo-request, time-exceeded } accept",
                "udp sport 67 udp dport 68 accept",
            ]
        );
        // `ct state invalid drop` is not an accept, and the commented-out ssh
        // rule is a comment however much it looks like a rule.
        assert!(!rules.iter().any(|r| r.contains("drop")));
        assert!(!rules.iter().any(|r| r.contains("dport 22")));
    }

    #[test]
    fn allowing_a_port_adds_a_block_and_changes_nothing_else() {
        let spec = PortSpec { proto: Proto::Tcp, port: 22 };
        let AllowOutcome::Added(updated) = allow_in(SAMPLE, spec).unwrap() else {
            panic!("the sample does not allow 22 yet");
        };

        assert!(updated.contains("\t\ttcp dport 22 accept\n"));
        assert!(updated.contains(BEGIN_MARKER));
        assert!(updated.contains(END_MARKER));
        assert!(updated.ends_with('\n'));

        // Every original line survives, in its original order. This is the
        // whole promise the marker block makes.
        let mut original = SAMPLE.lines();
        for line in updated.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if line.trim().starts_with('#') && line.contains("raven-firewall") {
                continue;
            }
            if line.trim() == "tcp dport 22 accept" {
                continue;
            }
            // The block's own explanatory comments.
            if line.trim().starts_with("# everything outside")
                || line.trim().starts_with("# line here")
                || line.trim().starts_with("# Everything between")
            {
                continue;
            }
            loop {
                let next = original.next().expect("updated has a line the original lacked");
                if next.trim().is_empty() {
                    continue;
                }
                assert_eq!(next, line);
                break;
            }
        }
    }

    #[test]
    fn the_new_rule_lands_inside_the_input_chain() {
        let spec = PortSpec { proto: Proto::Udp, port: 5353 };
        let AllowOutcome::Added(updated) = allow_in(SAMPLE, spec).unwrap() else {
            panic!("the sample does not allow 5353/udp yet");
        };
        assert!(accept_rules(&updated).contains(&"udp dport 5353 accept".to_string()));
        assert_eq!(input_policy(&updated).as_deref(), Some("drop"));
    }

    #[test]
    fn a_second_port_joins_the_block_rather_than_starting_another() {
        let AllowOutcome::Added(once) =
            allow_in(SAMPLE, PortSpec { proto: Proto::Tcp, port: 22 }).unwrap()
        else {
            panic!("first allow");
        };
        let AllowOutcome::Added(twice) =
            allow_in(&once, PortSpec { proto: Proto::Tcp, port: 80 }).unwrap()
        else {
            panic!("second allow");
        };

        assert_eq!(twice.matches(BEGIN_MARKER).count(), 1);
        assert_eq!(twice.matches(END_MARKER).count(), 1);
        // In the order they were opened.
        let rules = accept_rules(&twice);
        let at22 = rules.iter().position(|r| r == "tcp dport 22 accept").unwrap();
        let at80 = rules.iter().position(|r| r == "tcp dport 80 accept").unwrap();
        assert!(at22 < at80);
    }

    #[test]
    fn allowing_the_same_port_twice_is_uneventful() {
        let AllowOutcome::Added(once) =
            allow_in(SAMPLE, PortSpec { proto: Proto::Tcp, port: 22 }).unwrap()
        else {
            panic!("first allow");
        };
        assert_eq!(
            allow_in(&once, PortSpec { proto: Proto::Tcp, port: 22 }).unwrap(),
            AllowOutcome::AlreadyManaged
        );
    }

    #[test]
    fn a_hand_written_rule_is_reported_and_not_duplicated() {
        let by_hand = SAMPLE.replace("\t\tct state invalid drop", "\t\ttcp dport 631 accept");
        assert_eq!(
            allow_in(&by_hand, PortSpec { proto: Proto::Tcp, port: 631 }).unwrap(),
            AllowOutcome::AlreadyByHand("tcp dport 631 accept".to_string())
        );
    }

    /// The DHCP rule names port 68 and does not open it: it matches only a
    /// packet from a server's port 67. Treating that as "already allowed"
    /// would be this program telling someone a port is open when it is not.
    #[test]
    fn a_conditional_rule_does_not_count_as_opening_the_port() {
        let outcome = allow_in(SAMPLE, PortSpec { proto: Proto::Udp, port: 68 }).unwrap();
        assert!(matches!(outcome, AllowOutcome::Added(_)));
    }

    #[test]
    fn a_file_with_no_input_chain_is_refused_rather_than_mangled() {
        let no_chain = "table inet filter {\n\tchain output {\n\t}\n}\n";
        assert!(allow_in(no_chain, PortSpec { proto: Proto::Tcp, port: 22 }).is_err());
    }

    #[test]
    fn a_space_indented_file_stays_space_indented() {
        let spaces = "table inet filter {\n  chain input {\n    type filter hook input priority filter; policy drop;\n  }\n}\n";
        let AllowOutcome::Added(updated) =
            allow_in(spaces, PortSpec { proto: Proto::Tcp, port: 22 }).unwrap()
        else {
            panic!("allow");
        };
        assert!(updated.contains("\n    tcp dport 22 accept\n"));
        assert!(!updated.contains('\t'));
    }

    /// The file this repo actually ships has to survive the edit, because it
    /// is the file every one of these verbs will be pointed at.
    #[test]
    fn the_shipped_ruleset_can_be_read_and_added_to() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../etc/nftables.conf");
        let text = std::fs::read_to_string(&path).expect("etc/nftables.conf is readable");

        assert_eq!(input_policy(&text).as_deref(), Some("drop"));
        let rules = accept_rules(&text);
        assert!(rules.iter().any(|r| r == "ct state established,related accept"));
        assert!(rules.iter().any(|r| r == "iif \"lo\" accept"));
        // The ssh and CUPS lines are comments in the shipped file and must
        // read as comments here too.
        assert!(!rules.iter().any(|r| r.contains("dport 22")));
        assert!(!rules.iter().any(|r| r.contains("dport 631")));

        let AllowOutcome::Added(updated) =
            allow_in(&text, PortSpec { proto: Proto::Tcp, port: 22 }).unwrap()
        else {
            panic!("the shipped ruleset does not allow 22 yet");
        };
        assert!(accept_rules(&updated).contains(&"tcp dport 22 accept".to_string()));
        // The forward and output chains, and the whole header essay, are
        // untouched.
        assert!(updated.contains("chain forward {"));
        assert!(updated.contains("policy accept;"));
        // Exactly one `flush ruleset` *directive*. The header essay mentions
        // the phrase too, which is why this counts code lines rather than
        // substrings -- and is a reminder that everything in this file which
        // looks like a rule has to be checked against `code_of` first.
        assert_eq!(
            updated
                .lines()
                .filter(|line| code_of(line).trim() == "flush ruleset")
                .count(),
            1
        );
    }
}
