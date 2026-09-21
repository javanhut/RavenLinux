//! Where face templates live: `/var/lib/raven-face/<account>/<id>.face`.
//!
//! # Why here and not with the policy
//!
//! `ravend` keeps a file per account saying *where* a face may be used. This
//! daemon keeps the templates themselves, and the two are deliberately in
//! different places owned by different programs. The process that reads
//! `/etc/shadow` does not also hold the biometrics; the process that holds the
//! biometrics cannot start a session, cannot read a password hash, and does
//! not know what a match will be allowed to do.
//!
//! # What a template is
//!
//! A handful of 128-dimensional float vectors -- what the recogniser makes of
//! a face, not a picture of one. No image is ever written to disk by this
//! daemon, at any point, including during enrolment.
//!
//! An embedding is not a photograph and it is not a hash either. It cannot be
//! looked at, and it is not reversible into a recognisable face by anything
//! simple, but it is biometric data about a specific person and it is stable
//! for years. So the directory is `0700` root and the files are `0600`, and
//! this module is the only thing that writes them.
//!
//! It is not encrypted at rest, and pretending otherwise would be the
//! dishonest option: the only key this machine could keep is one this machine
//! could read, and anybody who can read a `0600` file owned by root on a
//! running system can read that key too. What protects these is the same thing
//! that protects `/etc/shadow`.

use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Where the templates live.
pub const DIR: &str = "/var/lib/raven-face";

/// The first bytes of every template file: a magic and a version, so a file
/// from a future format is refused rather than read as garbage floats.
const MAGIC: &[u8; 12] = b"RAVENFACE\x00\x01\n";

/// How many embeddings one look may hold. A ceiling on what a damaged file can
/// ask this to allocate, and comfortably above what an enrolment stores.
const MAX_VECTORS: usize = 32;

/// The recogniser's output size. Checked on read, because a file whose vectors
/// are the wrong length would compare against everything at once.
pub const DIM: usize = 128;

/// One stored look.
#[derive(Debug, Clone)]
pub struct Look {
    pub id: u8,
    pub label: String,
    /// Unix seconds.
    pub added: i64,
    /// One per good capture, each already L2-normalised.
    pub vectors: Vec<[f32; DIM]>,
}

/// Whether `name` is safe to use as a directory name.
///
/// The same rule `raven-greet-proto::valid_account` states, repeated here
/// rather than shared because these two live in different repositories and a
/// daemon that takes an account name over a socket must not depend on the
/// caller having checked it. Both sides check; that is the point.
pub fn valid_account(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && !name.starts_with(['.', '-'])
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'$'))
}

fn account_dir(root: &Path, account: &str) -> io::Result<PathBuf> {
    if !valid_account(account) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{account:?} is not an account name that can be stored"),
        ));
    }
    Ok(root.join(account))
}

/// Every look `account` has, lowest id first. An account with none is an empty
/// list and not an error.
pub fn load(root: &Path, account: &str) -> io::Result<Vec<Look>> {
    let dir = account_dir(root, account)?;
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    };
    let mut looks = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("face") {
            continue;
        }
        let Some(id) = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u8>().ok())
        else {
            continue;
        };
        match read_one(&path, id) {
            Ok(look) => looks.push(look),
            // One damaged file must not hide the rest: somebody with three
            // looks and one bad file should still be able to log in with the
            // other two, and then remove the bad one.
            Err(e) => log::warn!("ignoring {}: {e}", path.display()),
        }
    }
    looks.sort_by_key(|look| look.id);
    Ok(looks)
}

fn read_one(path: &Path, id: u8) -> io::Result<Look> {
    let mut file = std::fs::File::open(path)?;
    let mut magic = [0u8; MAGIC.len()];
    file.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "not a template this version writes",
        ));
    }

    let mut u16buf = [0u8; 2];
    let mut read_u16 = |file: &mut std::fs::File| -> io::Result<u16> {
        file.read_exact(&mut u16buf)?;
        Ok(u16::from_le_bytes(u16buf))
    };

    let label_len = read_u16(&mut file)? as usize;
    if label_len > 256 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "label too long"));
    }
    let mut label = vec![0u8; label_len];
    file.read_exact(&mut label)?;
    let label = String::from_utf8_lossy(&label).into_owned();

    let mut i64buf = [0u8; 8];
    file.read_exact(&mut i64buf)?;
    let added = i64::from_le_bytes(i64buf);

    let count = read_u16(&mut file)? as usize;
    let dim = read_u16(&mut file)? as usize;
    if dim != DIM {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("templates are {dim} long, not {DIM}"),
        ));
    }
    if count == 0 || count > MAX_VECTORS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{count} vectors is not a number of vectors"),
        ));
    }

    let mut bytes = vec![0u8; count * dim * 4];
    file.read_exact(&mut bytes)?;
    let mut vectors = Vec::with_capacity(count);
    for chunk in bytes.chunks_exact(dim * 4) {
        let mut v = [0f32; DIM];
        for (slot, four) in v.iter_mut().zip(chunk.chunks_exact(4)) {
            *slot = f32::from_le_bytes([four[0], four[1], four[2], four[3]]);
        }
        // A vector with a NaN in it compares equal to nothing and greater than
        // nothing, which would make a match silently impossible rather than
        // loudly broken.
        if v.iter().any(|f| !f.is_finite()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "a template holds something that is not a number",
            ));
        }
        vectors.push(v);
    }

    Ok(Look {
        id,
        label,
        added,
        vectors,
    })
}

/// Store a look under the lowest free id, and return it.
pub fn save(root: &Path, account: &str, label: &str, vectors: Vec<[f32; DIM]>) -> io::Result<Look> {
    if vectors.is_empty() || vectors.len() > MAX_VECTORS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a look needs between one and a handful of captures",
        ));
    }
    let dir = account_dir(root, account)?;
    std::fs::create_dir_all(root)?;
    std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700))?;
    std::fs::create_dir_all(&dir)?;
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;

    let taken: Vec<u8> = load(root, account)?.iter().map(|l| l.id).collect();
    let id = (1..=u8::MAX)
        .find(|id| !taken.contains(id))
        .ok_or_else(|| io::Error::other("this account has no free template slot"))?;

    let label = label
        .chars()
        .filter(|c| !c.is_control())
        .take(48)
        .collect::<String>()
        .trim()
        .to_string();
    let added = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let mut body = Vec::with_capacity(MAGIC.len() + 16 + vectors.len() * DIM * 4);
    body.extend_from_slice(MAGIC);
    body.extend_from_slice(&(label.len() as u16).to_le_bytes());
    body.extend_from_slice(label.as_bytes());
    body.extend_from_slice(&added.to_le_bytes());
    body.extend_from_slice(&(vectors.len() as u16).to_le_bytes());
    body.extend_from_slice(&(DIM as u16).to_le_bytes());
    for vector in &vectors {
        for value in vector {
            body.extend_from_slice(&value.to_le_bytes());
        }
    }

    // Through a temporary file and a rename: a crash halfway would otherwise
    // leave a truncated template, which reads as a damaged one and costs
    // somebody a face they thought they had enrolled.
    let path = dir.join(format!("{id}.face"));
    let tmp = dir.join(format!(".{id}.face.tmp"));
    {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?;
        file.write_all(&body)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(Look {
        id,
        label,
        added,
        vectors,
    })
}

/// Remove one of `account`'s looks. Removing one that is not there is the
/// state the caller asked for, so it is not an error.
pub fn forget(root: &Path, account: &str, id: u8) -> io::Result<()> {
    let dir = account_dir(root, account)?;
    match std::fs::remove_file(dir.join(format!("{id}.face"))) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// Remove every one of `account`'s looks, and nobody else's.
pub fn forget_all(root: &Path, account: &str) -> io::Result<()> {
    let dir = account_dir(root, account)?;
    match std::fs::remove_dir_all(&dir) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// How alike two normalised vectors are: 1.0 is identical, 0.0 unrelated.
///
/// A plain dot product, because both sides are already unit length -- see
/// [`normalise`]. Not constant-time, and deliberately not: there is no secret
/// here to leak through timing. The templates are root's, the comparison
/// happens inside this process, and what crosses the socket is one bit.
pub fn similarity(a: &[f32; DIM], b: &[f32; DIM]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// A vector scaled to unit length, or `None` if it has no length to scale.
pub fn normalise(mut v: [f32; DIM]) -> Option<[f32; DIM]> {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if !norm.is_finite() || norm < f32::EPSILON {
        return None;
    }
    for x in &mut v {
        *x /= norm;
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("raven-faced-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn vector(seed: f32) -> [f32; DIM] {
        let mut v = [0f32; DIM];
        for (i, slot) in v.iter_mut().enumerate() {
            *slot = (i as f32 * 0.01 + seed).sin();
        }
        normalise(v).expect("has length")
    }

    #[test]
    fn a_look_round_trips_and_is_private() {
        let root = scratch("round-trip");
        let saved = save(&root, "javan", "with glasses", vec![vector(0.0), vector(1.0)])
            .expect("saves");
        assert_eq!(saved.id, 1);

        let looks = load(&root, "javan").expect("loads");
        assert_eq!(looks.len(), 1);
        assert_eq!(looks[0].label, "with glasses");
        assert_eq!(looks[0].vectors.len(), 2);
        assert!(similarity(&looks[0].vectors[0], &saved.vectors[0]) > 0.999);

        let mode = |p: &Path| std::fs::metadata(p).expect("exists").permissions().mode() & 0o777;
        assert_eq!(mode(&root), 0o700);
        assert_eq!(mode(&root.join("javan")), 0o700);
        assert_eq!(mode(&root.join("javan/1.face")), 0o600);
    }

    #[test]
    fn ids_are_handed_out_lowest_free_first() {
        let root = scratch("ids");
        for expected in 1..=3 {
            assert_eq!(
                save(&root, "javan", "", vec![vector(expected as f32)])
                    .expect("saves")
                    .id,
                expected
            );
        }
        forget(&root, "javan", 2).expect("forgets");
        assert_eq!(save(&root, "javan", "", vec![vector(9.0)]).expect("saves").id, 2);
    }

    /// One account's templates are one account's. This is the test that would
    /// fail if `forget_all` ever grew a shortcut through the root directory.
    #[test]
    fn forgetting_one_account_leaves_the_others_alone() {
        let root = scratch("apart");
        save(&root, "javan", "", vec![vector(0.0)]).expect("saves");
        save(&root, "somebody", "", vec![vector(1.0)]).expect("saves");
        forget_all(&root, "javan").expect("forgets");
        assert!(load(&root, "javan").expect("loads").is_empty());
        assert_eq!(load(&root, "somebody").expect("loads").len(), 1);
    }

    #[test]
    fn a_name_that_is_a_path_is_refused() {
        let root = scratch("traversal");
        for bad in ["../escape", "a/b", ".hidden", "", "-rf"] {
            assert!(save(&root, bad, "", vec![vector(0.0)]).is_err(), "{bad:?}");
            assert!(load(&root, bad).is_err(), "{bad:?}");
            assert!(forget_all(&root, bad).is_err(), "{bad:?}");
        }
    }

    /// A damaged file is skipped and the good ones still load, so one bad
    /// template does not cost somebody every face they have.
    #[test]
    fn a_damaged_file_does_not_hide_the_good_ones() {
        let root = scratch("damaged");
        save(&root, "javan", "good", vec![vector(0.0)]).expect("saves");
        std::fs::write(root.join("javan/2.face"), b"nonsense").expect("writes");
        let looks = load(&root, "javan").expect("loads");
        assert_eq!(looks.len(), 1);
        assert_eq!(looks[0].label, "good");
    }

    /// A vector the wrong length would compare against everything at once.
    #[test]
    fn a_template_of_the_wrong_size_is_refused() {
        let root = scratch("dim");
        save(&root, "javan", "", vec![vector(0.0)]).expect("saves");
        let path = root.join("javan/1.face");
        let mut bytes = std::fs::read(&path).expect("reads");
        // The dim field: after the magic, the label length, the label, the
        // timestamp and the count.
        let at = MAGIC.len() + 2 + 0 + 8 + 2;
        bytes[at..at + 2].copy_from_slice(&64u16.to_le_bytes());
        std::fs::write(&path, &bytes).expect("writes");
        assert!(load(&root, "javan").expect("loads").is_empty());
    }

    #[test]
    fn similarity_is_one_for_the_same_face_and_less_for_another() {
        let a = vector(0.0);
        let b = vector(2.0);
        assert!((similarity(&a, &a) - 1.0).abs() < 1e-5);
        assert!(similarity(&a, &b) < 0.999);
    }

    #[test]
    fn a_vector_with_no_length_does_not_normalise_to_nan() {
        assert!(normalise([0.0; DIM]).is_none());
        assert!(normalise([f32::NAN; DIM]).is_none());
    }
}
