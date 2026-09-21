//! Deciding whether the face in front of the camera is a face.
//!
//! # What this is, and what it is not
//!
//! An ordinary camera cannot tell a face from a photograph of one. That is not
//! a limitation of any particular recogniser; the two produce the same pixels,
//! which is the whole point of a photograph. So face unlock on a machine
//! without an infrared sensor needs something the camera does not have on its
//! own, and what this machine has is a screen.
//!
//! The screen throws a short, random sequence of colours at whoever is in
//! front of it, and this module reads three things out of the frames that come
//! back:
//!
//! 1. **Does the face respond at all, and in time?** A recording being played
//!    back on a phone does not know what colour the screen is about to turn.
//!    The sequence is picked fresh for every attempt from `/dev/urandom`, so
//!    there is nothing to have recorded.
//!
//! 2. **Does it respond in three dimensions?** This is the one that catches a
//!    printed photograph, which *does* reflect the flash -- faithfully, and
//!    flatly. A real face has a nose. Light arriving from a screen at arm's
//!    length lands unevenly on it: the forehead and the bridge catch more, the
//!    eye sockets and the underside of the chin much less. A sheet of paper
//!    lights up all over at once. So the response is measured per region and
//!    what is looked at is how *unevenly* it changed, not how much.
//!
//! 3. **Is it a mirror?** Glossy paper and phone glass throw the flash back as
//!    a bright spot rather than absorbing and re-emitting it the way skin
//!    does. One region far brighter than the rest is a reflector.
//!
//! # What it does not stop
//!
//! A well-made three-dimensional mask. It has a nose, it is not glossy, and it
//! responds to light like a face because it is shaped like one. Nothing short
//! of an infrared sensor or a depth camera addresses that, and this module
//! does not pretend to.
//!
//! It is also not a model. There is no trained anti-spoofing network here, and
//! that is a deliberate decision rather than an omission: the OpenCV Zoo,
//! which is where the other two models come from with pinned hashes and a
//! licence, has no anti-spoofing model in it. The ones that circulate are
//! conversions of a research checkpoint, from no particular publisher, with no
//! hash anybody vouches for. Downloading one of those into the process that
//! decides who may log in would buy a number at the cost of the only property
//! that makes the other two acceptable. The seam is here if one ever becomes
//! available: [`Verdict`] is what the rest of the daemon consumes, and a model
//! would be another input to [`judge`].
//!
//! # The thresholds
//!
//! Every constant below is a judgement, not a measurement. They are set where
//! a live face in ordinary indoor light passes comfortably and a printed
//! photograph does not, and they are all in `/etc/raven/face.toml` so that a
//! machine where they are wrong can be fixed without a rebuild. The failure
//! this errs towards is refusing a real face, because the password is right
//! there underneath and a refusal costs somebody two seconds.

use std::io::Read;

/// A colour the screen is asked to paint.
pub type Colour = [u8; 3];

/// How many colours one challenge has. Each costs its own dwell time, so this
/// is the whole reason the check takes about three quarters of a second.
const STEPS: usize = 5;

/// How long each colour is held. Long enough for the panel to have finished
/// changing and for at least one frame to have been captured under it at 30fps,
/// with room for a frame to be late.
pub const DWELL: std::time::Duration = std::time::Duration::from_millis(140);

/// How long after a colour goes up before frames start counting.
///
/// The panel takes a few milliseconds, the compositor takes a frame, and the
/// camera's own pipeline holds a frame or two. Frames from inside this window
/// show the *previous* colour and would smear every step into the next.
pub const SETTLE: std::time::Duration = std::time::Duration::from_millis(60);

/// The face's brightness has to move by at least this much across the
/// sequence, as a fraction of full scale, for anything to be measurable.
///
/// Below it the screen is not reaching them: too far away, or a room bright
/// enough to swamp a laptop panel. That is not a spoof and is not reported as
/// one.
const MIN_RESPONSE: f32 = 0.015;

/// How closely the face's brightness has to follow the sequence, per channel.
const MIN_CORRELATION: f32 = 0.55;

/// How unevenly the light has to land across the face. The flatness test; see
/// the module note.
const MIN_RELIEF: f32 = 0.10;

/// How much brighter than the rest one region may be before the thing in front
/// of the camera is a mirror rather than a face.
const MAX_GLARE: f32 = 4.0;

/// The face is split this many ways along each axis for the relief measure.
pub const GRID: usize = 3;

/// One attempt's sequence of colours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    pub steps: Vec<Colour>,
}

impl Challenge {
    /// A fresh sequence, from `/dev/urandom`.
    ///
    /// Unpredictable per attempt, which is the property the whole check rests
    /// on: a sequence somebody could guess is a sequence somebody could have
    /// recorded in advance. If the kernel's randomness cannot be read -- which
    /// on a running Linux system means something is very wrong -- this returns
    /// `None` and the caller refuses the watch rather than running a
    /// predictable challenge.
    pub fn new() -> Option<Self> {
        let mut bytes = [0u8; STEPS];
        std::fs::File::open("/dev/urandom")
            .and_then(|mut f| f.read_exact(&mut bytes))
            .map_err(|e| log::error!("cannot read /dev/urandom: {e}"))
            .ok()?;

        // Saturated primaries and white, because what is being measured is the
        // difference between steps and a pastel palette would measure less of
        // it. Black is the baseline: the face lit by the room alone.
        const PALETTE: [Colour; 4] = [
            [255, 40, 40],
            [40, 255, 40],
            [60, 60, 255],
            [255, 255, 255],
        ];
        const DARK: Colour = [0, 0, 0];

        let mut steps = Vec::with_capacity(STEPS);
        for (i, byte) in bytes.iter().enumerate() {
            // Every other step is dark, so each lit step is measured against a
            // baseline taken moments before it rather than against the start
            // of the sequence -- somebody who leans forward mid-check must not
            // fail it.
            steps.push(if i % 2 == 0 {
                DARK
            } else {
                PALETTE[(*byte as usize) % PALETTE.len()]
            });
        }
        Some(Self { steps })
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }

}

/// What one step of the sequence looked like on the face.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    /// Which step of the challenge this was taken under.
    pub step: usize,
    /// Mean BGR of each cell of the face, row-major.
    pub cells: [[f32; 3]; GRID * GRID],
}

impl Sample {
    /// The whole face's mean, per channel.
    fn face(&self) -> [f32; 3] {
        let mut sum = [0f32; 3];
        for cell in &self.cells {
            for (s, c) in sum.iter_mut().zip(cell) {
                *s += c;
            }
        }
        sum.map(|s| s / (GRID * GRID) as f32)
    }

    /// The whole face's mean brightness.
    fn brightness(&self) -> f32 {
        let [b, g, r] = self.face();
        (b + g + r) / 3.0
    }
}

/// What the check decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// A live face. Not proof; see the module note on masks.
    Live,
    /// Something that is not one. Counted as a miss by the caller and logged.
    Spoof,
    /// The check could not run: the screen is not reaching them, or not enough
    /// frames arrived. Carries the word for a correction to put on screen --
    /// this is not a failure anybody should be blamed for.
    Unsure(&'static str),
}

/// Judge one attempt.
///
/// `samples` is every measurement taken during the sequence, in any order and
/// with any number per step; steps with no sample at all are simply missing,
/// which is ordinary on a camera that dropped a frame.
pub fn judge(challenge: &Challenge, samples: &[Sample], limits: Limits) -> Verdict {
    // Averaged per step first: several frames under one colour are several
    // looks at the same thing, and the mean of them is less noisy than any.
    let mut per_step: Vec<Option<Sample>> = vec![None; challenge.len()];
    let mut counts = vec![0u32; challenge.len()];
    for sample in samples {
        let Some(slot) = per_step.get_mut(sample.step) else {
            continue;
        };
        match slot {
            None => *slot = Some(*sample),
            Some(running) => {
                let n = counts[sample.step] as f32;
                for (r, s) in running.cells.iter_mut().zip(sample.cells.iter()) {
                    for (rc, sc) in r.iter_mut().zip(s.iter()) {
                        *rc = (*rc * n + sc) / (n + 1.0);
                    }
                }
            }
        }
        counts[sample.step] += 1;
    }

    let seen: Vec<(usize, Sample)> = per_step
        .iter()
        .enumerate()
        .filter_map(|(i, s)| s.map(|s| (i, s)))
        .collect();
    // Two lit steps and two dark ones is the least this can say anything with.
    if seen.len() < 4 {
        return Verdict::Unsure("still");
    }

    // --- 1. did it respond at all -----------------------------------------
    let brightnesses: Vec<f32> = seen.iter().map(|(_, s)| s.brightness()).collect();
    let low = brightnesses.iter().copied().fold(f32::INFINITY, f32::min);
    let high = brightnesses.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    if high - low < limits.min_response {
        // The room is winning. Not a spoof: a photograph in the same room
        // would not respond either, but neither would its owner, and the one
        // of them standing there is the owner.
        return Verdict::Unsure("bright");
    }

    // --- 2. did it respond to *this* sequence -----------------------------
    // Per channel, because a red flash and a blue one of the same brightness
    // must not look the same: a screen replaying a recording brightens and
    // dims plausibly and gets the colours wrong.
    for channel in 0..3 {
        // The challenge is in RGB and the camera in BGR.
        let asked: Vec<f32> = seen
            .iter()
            .map(|(step, _)| f32::from(challenge.steps[*step][2 - channel]) / 255.0)
            .collect();
        let got: Vec<f32> = seen.iter().map(|(_, s)| s.face()[channel]).collect();
        let r = correlation(&asked, &got);
        if r < limits.min_correlation {
            log::debug!("channel {channel} followed the sequence at r={r:.2}");
            return Verdict::Spoof;
        }
    }

    // --- 3. did it respond in three dimensions ----------------------------
    // The difference between the brightest lit step and the darkest, per cell.
    // A face's nose and forehead move much further than its eye sockets; a
    // sheet of paper moves everywhere at once.
    let brightest = seen
        .iter()
        .max_by(|a, b| a.1.brightness().total_cmp(&b.1.brightness()))
        .map(|(_, s)| *s);
    let darkest = seen
        .iter()
        .min_by(|a, b| a.1.brightness().total_cmp(&b.1.brightness()))
        .map(|(_, s)| *s);
    let (Some(brightest), Some(darkest)) = (brightest, darkest) else {
        return Verdict::Unsure("still");
    };

    let mut deltas = [0f32; GRID * GRID];
    for (i, delta) in deltas.iter_mut().enumerate() {
        let lit: f32 = brightest.cells[i].iter().sum::<f32>() / 3.0;
        let dark: f32 = darkest.cells[i].iter().sum::<f32>() / 3.0;
        *delta = (lit - dark).max(0.0);
    }
    let mean = deltas.iter().sum::<f32>() / deltas.len() as f32;
    if mean < f32::EPSILON {
        return Verdict::Unsure("bright");
    }

    let variance = deltas.iter().map(|d| (d - mean).powi(2)).sum::<f32>() / deltas.len() as f32;
    let relief = variance.sqrt() / mean;
    if relief < limits.min_relief {
        log::debug!("the light landed flat: relief {relief:.2}");
        return Verdict::Spoof;
    }

    // --- 4. or is it a mirror ---------------------------------------------
    let mut sorted = deltas;
    sorted.sort_by(f32::total_cmp);
    let median = sorted[sorted.len() / 2];
    let brightest_cell = sorted[sorted.len() - 1];
    if median > f32::EPSILON && brightest_cell / median > limits.max_glare {
        log::debug!("one region caught {:.1}x the rest", brightest_cell / median);
        return Verdict::Spoof;
    }

    Verdict::Live
}

/// The numbers [`judge`] compares against, so a machine can be tuned without a
/// rebuild. See the module note: these are judgements, not measurements.
#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub min_response: f32,
    pub min_correlation: f32,
    pub min_relief: f32,
    pub max_glare: f32,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            min_response: MIN_RESPONSE,
            min_correlation: MIN_CORRELATION,
            min_relief: MIN_RELIEF,
            max_glare: MAX_GLARE,
        }
    }
}

/// Pearson's, or `0.0` for a series that does not vary.
fn correlation(a: &[f32], b: &[f32]) -> f32 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let mean = |v: &[f32]| v[..n].iter().sum::<f32>() / n as f32;
    let (ma, mb) = (mean(a), mean(b));
    let (mut cov, mut va, mut vb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..n {
        let (da, db) = (a[i] - ma, b[i] - mb);
        cov += da * db;
        va += da * da;
        vb += db * db;
    }
    if va < f32::EPSILON || vb < f32::EPSILON {
        return 0.0;
    }
    cov / (va.sqrt() * vb.sqrt())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn challenge() -> Challenge {
        Challenge {
            steps: vec![
                [0, 0, 0],
                [255, 40, 40],
                [0, 0, 0],
                [60, 60, 255],
                [0, 0, 0],
            ],
        }
    }

    /// Build samples for a thing in front of the camera: `relief` is how much
    /// more the centre cells catch than the edges (a face has some, paper has
    /// none), `follows` is whether it reflects the colour that was asked for.
    fn samples(challenge: &Challenge, relief: f32, follows: bool, glare: f32) -> Vec<Sample> {
        let mut out = Vec::new();
        for (step, colour) in challenge.steps.iter().enumerate() {
            let mut cells = [[0.20f32; 3]; GRID * GRID];
            for (i, cell) in cells.iter_mut().enumerate() {
                // The centre cell stands proud, the corners are shaded.
                let shape = 1.0 + relief * (if i == 4 { 1.0 } else { -0.25 });
                for (channel, value) in cell.iter_mut().enumerate() {
                    let asked = f32::from(colour[2 - channel]) / 255.0;
                    // A recording on a phone is not still -- it brightens and
                    // dims as the video plays. It just does not do it in step
                    // with a sequence it could not have known.
                    let lit = if follows {
                        asked
                    } else {
                        [0.9, 0.1, 0.35, 0.8, 0.2][step % 5]
                    };
                    *value = 0.20 + 0.10 * lit * shape;
                }
            }
            // One cell acting as a mirror.
            if glare > 1.0 {
                for value in &mut cells[0] {
                    *value = 0.20 + (*value - 0.20) * glare;
                }
            }
            out.push(Sample { step, cells });
        }
        out
    }

    #[test]
    fn a_face_that_follows_the_sequence_in_relief_is_live() {
        let c = challenge();
        let s = samples(&c, 0.8, true, 1.0);
        assert_eq!(judge(&c, &s, Limits::default()), Verdict::Live);
    }

    /// The one this whole module exists for: a printed photograph reflects the
    /// sequence perfectly and reflects it flat.
    #[test]
    fn a_flat_thing_that_follows_the_sequence_is_a_spoof() {
        let c = challenge();
        let s = samples(&c, 0.0, true, 1.0);
        assert_eq!(judge(&c, &s, Limits::default()), Verdict::Spoof);
    }

    /// A recording being played back brightens and dims and gets the colours
    /// wrong, because it does not know what they are going to be.
    #[test]
    fn something_that_ignores_the_sequence_is_a_spoof() {
        let c = challenge();
        let s = samples(&c, 0.8, false, 1.0);
        assert_eq!(judge(&c, &s, Limits::default()), Verdict::Spoof);
    }

    #[test]
    fn a_mirror_is_a_spoof() {
        let c = challenge();
        let s = samples(&c, 0.8, true, 9.0);
        assert_eq!(judge(&c, &s, Limits::default()), Verdict::Spoof);
    }

    /// A room brighter than the screen is not somebody's fault, and must not
    /// be reported as an attack.
    #[test]
    fn a_face_the_screen_cannot_reach_is_unsure_and_not_a_spoof() {
        let c = challenge();
        let flat: Vec<Sample> = (0..c.len())
            .map(|step| Sample {
                step,
                cells: [[0.9; 3]; GRID * GRID],
            })
            .collect();
        assert_eq!(
            judge(&c, &flat, Limits::default()),
            Verdict::Unsure("bright")
        );
    }

    #[test]
    fn too_few_frames_is_unsure() {
        let c = challenge();
        let s = samples(&c, 0.8, true, 1.0);
        assert!(matches!(
            judge(&c, &s[..2], Limits::default()),
            Verdict::Unsure(_)
        ));
    }

    /// Several frames under one colour are averaged rather than counted twice.
    #[test]
    fn repeated_samples_for_one_step_are_averaged() {
        let c = challenge();
        let mut s = samples(&c, 0.8, true, 1.0);
        let extra = s.clone();
        s.extend(extra);
        assert_eq!(judge(&c, &s, Limits::default()), Verdict::Live);
    }

    /// A sample naming a step that is not in the challenge is dropped, not a
    /// panic: it is the shape a daemon bug or a race would take.
    #[test]
    fn a_sample_out_of_range_is_ignored() {
        let c = challenge();
        let mut s = samples(&c, 0.8, true, 1.0);
        s.push(Sample {
            step: 99,
            cells: [[1.0; 3]; GRID * GRID],
        });
        assert_eq!(judge(&c, &s, Limits::default()), Verdict::Live);
    }

    /// Two attempts in a row must not ask for the same colours, or the whole
    /// thing is a recording waiting to be made.
    #[test]
    fn the_sequence_is_not_the_same_twice() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..24 {
            if let Some(c) = Challenge::new() {
                assert_eq!(c.len(), STEPS);
                // Every other step is the baseline, by construction.
                assert_eq!(c.steps[0], [0, 0, 0]);
                seen.insert(c.steps.clone());
            }
        }
        assert!(seen.len() > 1, "the challenge is predictable");
    }

    #[test]
    fn correlation_of_a_flat_series_is_zero_and_not_a_nan() {
        assert_eq!(correlation(&[1.0, 1.0, 1.0], &[1.0, 2.0, 3.0]), 0.0);
        assert_eq!(correlation(&[1.0], &[1.0]), 0.0);
        assert!((correlation(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]) - 1.0).abs() < 1e-5);
    }
}
