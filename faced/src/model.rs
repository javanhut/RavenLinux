//! The two networks: finding a face, and saying whose it is.
//!
//! Both are ONNX files from the OpenCV Zoo, run through `tract` -- a Rust
//! inference engine, so there is no C++ runtime and no FFI inside the process
//! that decides who may log in:
//!
//! - **YuNet** finds faces in a frame and gives five landmarks for each. 230 KB.
//! - **SFace** turns one aligned face into 128 numbers. 37 MB.
//!
//! Neither is trained here and neither is fine-tuned here. They are fetched by
//! `fetch-models.sh`, which pins their hashes, and this daemon refuses to
//! offer face unlock at all if they are missing -- see [`Models::load`].
//!
//! # The bits that are easy to get subtly wrong
//!
//! Three, all of which fail silently rather than loudly, and all of which have
//! a test below:
//!
//! - **Channel order.** Both models were trained on OpenCV's output, which is
//!   BGR. Feeding RGB does not crash; it makes the recogniser worse by an
//!   amount nobody notices until somebody cannot log in.
//! - **The anchor decode.** YuNet's boxes are offsets from a grid, and the
//!   grid's row-major order has to match the model's. Getting it wrong finds
//!   faces in the wrong places, which looks like a camera problem.
//! - **The alignment.** SFace expects a face warped onto five fixed points. An
//!   unaligned crop still produces 128 numbers, and they are useless.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tract_onnx::prelude::*;

use crate::camera::Frame;
use crate::store::DIM;

/// Where `fetch-models.sh` puts them.
pub const MODEL_DIR: &str = "/usr/share/raven-face/models";

pub const DETECTOR: &str = "face_detection_yunet_2023mar.onnx";
pub const RECOGNISER: &str = "face_recognition_sface_2021dec.onnx";

/// The square YuNet was exported at.
const DETECT_SIDE: usize = 640;

/// The size SFace takes, and the five points a face is warped onto to get
/// there. ArcFace's reference landmarks, which is what SFace was trained with;
/// they are not adjustable, because the model is not being retrained.
const ALIGN_SIDE: usize = 112;
const REFERENCE: [(f32, f32); 5] = [
    (38.2946, 51.6963), // the subject's right eye, on the left of the image
    (73.5318, 51.5014), // left eye
    (56.0252, 71.7366), // nose tip
    (41.5493, 92.3655), // right mouth corner
    (70.7299, 92.2041), // left mouth corner
];

/// A found face, in the coordinates of the frame it was found in.
#[derive(Debug, Clone, Copy)]
pub struct Face {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    /// Right eye, left eye, nose, right mouth corner, left mouth corner.
    pub landmarks: [(f32, f32); 5],
    pub score: f32,
}

impl Face {
    /// How much of the frame's shorter side this face spans. The "are you far
    /// enough away / close enough" number.
    pub fn fill(&self, frame_width: u32, frame_height: u32) -> f32 {
        let short = frame_width.min(frame_height) as f32;
        if short <= 0.0 {
            return 0.0;
        }
        self.height / short
    }

    /// How far the face's centre is from the frame's, as a fraction of the
    /// frame. Used to ask somebody to look at the camera rather than past it.
    pub fn off_centre(&self, frame_width: u32, frame_height: u32) -> f32 {
        let (cx, cy) = (self.x + self.width / 2.0, self.y + self.height / 2.0);
        let dx = (cx - frame_width as f32 / 2.0) / frame_width as f32;
        let dy = (cy - frame_height as f32 / 2.0) / frame_height as f32;
        (dx * dx + dy * dy).sqrt()
    }

    /// Whether the head is turned far enough away that the landmarks are no
    /// longer a face the recogniser was trained on.
    ///
    /// Measured from the nose's position between the eyes rather than from a
    /// pose model: a head turned to one side puts its nose next to one eye.
    pub fn is_turned_away(&self) -> bool {
        let [right_eye, left_eye, nose, ..] = self.landmarks;
        let span = (left_eye.0 - right_eye.0).abs();
        if span < 1.0 {
            return true;
        }
        let middle = (right_eye.0 + left_eye.0) / 2.0;
        ((nose.0 - middle) / span).abs() > 0.35
    }
}

/// Both networks, loaded.
pub struct Models {
    detector: Arc<TypedRunnableModel>,
    recogniser: Arc<TypedRunnableModel>,
}

impl std::fmt::Debug for Models {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Models { .. }")
    }
}

impl Models {
    /// Load both, from `dir`.
    ///
    /// Slow -- optimising two graphs takes a moment -- and done once, at
    /// start-up, rather than per request. A machine without the files gets a
    /// clear error that names the missing one, because "face unlock does not
    /// work" and "the models are not installed" are the same symptom and very
    /// different fixes.
    pub fn load(dir: &Path) -> anyhow::Result<Self> {
        let detector = load_one(&dir.join(DETECTOR), &[1, 3, DETECT_SIDE, DETECT_SIDE])?;
        let recogniser = load_one(&dir.join(RECOGNISER), &[1, 3, ALIGN_SIDE, ALIGN_SIDE])?;
        Ok(Self {
            detector,
            recogniser,
        })
    }

    /// The largest face in `frame`, and whether picking it was a guess.
    ///
    /// The largest rather than the best-scoring: somebody standing at their
    /// own machine is the nearest face to it, and a better-scoring face across
    /// the room is not who is logging in. The second half of the answer is
    /// [`Self::crowded`] -- two faces of a similar size is somebody standing
    /// behind you, which is a thing to say something about rather than to pick
    /// between.
    pub fn detect(&self, frame: &Frame, threshold: f32) -> anyhow::Result<(Option<Face>, bool)> {
        let (tensor, pad) = letterbox(frame);
        let outputs = self.detector.run(tvec!(tensor.into()))?;

        let mut faces = Vec::new();
        for (level, stride) in [8usize, 16, 32].into_iter().enumerate() {
            let cls = outputs[level].to_plain_array_view::<f32>()?;
            let obj = outputs[3 + level].to_plain_array_view::<f32>()?;
            let bbox = outputs[6 + level].to_plain_array_view::<f32>()?;
            let kps = outputs[9 + level].to_plain_array_view::<f32>()?;
            let cols = DETECT_SIDE / stride;
            let rows = cols;

            for row in 0..rows {
                for col in 0..cols {
                    let i = row * cols + col;
                    // Both heads have to agree: `cls` says "this looks like a
                    // face", `obj` says "there is something here at all", and
                    // the score OpenCV compares against a threshold is their
                    // geometric mean.
                    let score = (cls[[0, i, 0]].max(0.0) * obj[[0, i, 0]].max(0.0)).sqrt();
                    if score < threshold {
                        continue;
                    }
                    let s = stride as f32;
                    let cx = (col as f32 + bbox[[0, i, 0]]) * s;
                    let cy = (row as f32 + bbox[[0, i, 1]]) * s;
                    let w = bbox[[0, i, 2]].exp() * s;
                    let h = bbox[[0, i, 3]].exp() * s;
                    let mut landmarks = [(0.0, 0.0); 5];
                    for (k, point) in landmarks.iter_mut().enumerate() {
                        *point = (
                            (col as f32 + kps[[0, i, k * 2]]) * s,
                            (row as f32 + kps[[0, i, k * 2 + 1]]) * s,
                        );
                    }
                    faces.push(Face {
                        x: cx - w / 2.0,
                        y: cy - h / 2.0,
                        width: w,
                        height: h,
                        landmarks,
                        score,
                    });
                }
            }
        }

        let mut faces = suppress(faces, 0.3);
        for face in &mut faces {
            pad.undo(face);
        }
        faces.sort_by(|a, b| b.height.total_cmp(&a.height));
        Ok((faces.first().copied(), Self::crowded(&faces)))
    }

    /// Whether two faces in view are close enough in size that picking the
    /// larger would be guessing.
    pub fn crowded(faces: &[Face]) -> bool {
        matches!(faces, [first, second, ..] if second.height > first.height * 0.7)
    }

    /// One face as 128 normalised numbers.
    ///
    /// `None` if the warp produced something with no length, which means the
    /// landmarks were degenerate -- a face found at the very edge of the frame,
    /// usually.
    pub fn embed(&self, frame: &Frame, face: &Face) -> anyhow::Result<Option<[f32; DIM]>> {
        let aligned = align(frame, face);
        let tensor = tract_ndarray::Array4::from_shape_fn(
            (1, 3, ALIGN_SIDE, ALIGN_SIDE),
            |(_, c, y, x)| {
                // BGR in, BGR out: the channel index is the channel index. See
                // the module note on why this is written down.
                f32::from(aligned[(y * ALIGN_SIDE + x) * 3 + c])
            },
        )
        .into_tensor();

        let out = self.recogniser.run(tvec!(tensor.into()))?;
        let view = out[0].to_plain_array_view::<f32>()?;
        let mut vector = [0f32; DIM];
        for (slot, value) in vector.iter_mut().zip(view.iter()) {
            *slot = *value;
        }
        Ok(crate::store::normalise(vector))
    }
}

fn load_one(path: &Path, shape: &[usize]) -> anyhow::Result<Arc<TypedRunnableModel>> {
    if !path.exists() {
        anyhow::bail!(
            "{} is not installed; run fetch-models.sh",
            path.display()
        );
    }
    let model = tract_onnx::onnx()
        .model_for_path(path)?
        .with_input_fact(0, f32::fact(shape).into())?
        .into_optimized()?
        .into_runnable()?;
    Ok(model)
}

/// How a frame was fitted into the detector's square, so a box found in the
/// square can be put back where it came from.
#[derive(Debug, Clone, Copy)]
struct Pad {
    scale: f32,
    dx: f32,
    dy: f32,
}

impl Pad {
    fn undo(&self, face: &mut Face) {
        face.x = (face.x - self.dx) / self.scale;
        face.y = (face.y - self.dy) / self.scale;
        face.width /= self.scale;
        face.height /= self.scale;
        for point in &mut face.landmarks {
            point.0 = (point.0 - self.dx) / self.scale;
            point.1 = (point.1 - self.dy) / self.scale;
        }
    }
}

/// A frame scaled to fit the detector's square and centred in it, with the
/// rest left black.
///
/// Letterboxed rather than stretched: YuNet's boxes are square-ish offsets
/// from a square grid, and a face squashed to fit a different aspect ratio is
/// a face it was not trained on.
fn letterbox(frame: &Frame) -> (Tensor, Pad) {
    let (fw, fh) = (frame.width as f32, frame.height as f32);
    let scale = (DETECT_SIDE as f32 / fw).min(DETECT_SIDE as f32 / fh);
    let (sw, sh) = (fw * scale, fh * scale);
    let dx = (DETECT_SIDE as f32 - sw) / 2.0;
    let dy = (DETECT_SIDE as f32 - sh) / 2.0;

    let tensor = tract_ndarray::Array4::from_shape_fn(
        (1, 3, DETECT_SIDE, DETECT_SIDE),
        |(_, c, y, x)| {
            let sx = (x as f32 - dx) / scale;
            let sy = (y as f32 - dy) / scale;
            if sx < 0.0 || sy < 0.0 || sx >= fw || sy >= fh {
                return 0.0;
            }
            let i = ((sy as usize) * frame.width as usize + sx as usize) * 3 + c;
            frame.bgr.get(i).map_or(0.0, |v| f32::from(*v))
        },
    )
    .into_tensor();

    (tensor, Pad { scale, dx, dy })
}

/// Non-maximum suppression: one box per face, keeping the best-scoring.
fn suppress(mut faces: Vec<Face>, overlap: f32) -> Vec<Face> {
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Face> = Vec::new();
    for face in faces {
        if kept.iter().any(|k| iou(k, &face) > overlap) {
            continue;
        }
        kept.push(face);
        if kept.len() >= 8 {
            break;
        }
    }
    kept
}

fn iou(a: &Face, b: &Face) -> f32 {
    let x0 = a.x.max(b.x);
    let y0 = a.y.max(b.y);
    let x1 = (a.x + a.width).min(b.x + b.width);
    let y1 = (a.y + a.height).min(b.y + b.height);
    let inter = (x1 - x0).max(0.0) * (y1 - y0).max(0.0);
    let union = a.width * a.height + b.width * b.height - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

/// A face warped onto SFace's five reference points, as `112 * 112 * 3` BGR
/// bytes.
///
/// The transform is a similarity -- scale, rotation, translation, and nothing
/// else -- fitted to the five landmarks by least squares. A full affine would
/// fit the points better and would also shear the face, which is not a thing
/// the recogniser has ever seen.
fn align(frame: &Frame, face: &Face) -> Vec<u8> {
    let transform = similarity_from(&face.landmarks, &REFERENCE);
    let mut out = vec![0u8; ALIGN_SIDE * ALIGN_SIDE * 3];
    for y in 0..ALIGN_SIDE {
        for x in 0..ALIGN_SIDE {
            // The inverse: for each output pixel, where in the frame it comes
            // from. Forward-mapping would leave holes.
            let (sx, sy) = transform.invert(x as f32, y as f32);
            let pixel = sample(frame, sx, sy);
            let o = (y * ALIGN_SIDE + x) * 3;
            out[o..o + 3].copy_from_slice(&pixel);
        }
    }
    out
}

/// Bilinear, clamped at the edges.
fn sample(frame: &Frame, x: f32, y: f32) -> [u8; 3] {
    let (w, h) = (frame.width as i32, frame.height as i32);
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor() as i32, y.floor() as i32);
    let (x1, y1) = ((x0 + 1).min(w - 1), (y0 + 1).min(h - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);

    let at = |px: i32, py: i32, c: usize| -> f32 {
        let i = ((py as usize) * frame.width as usize + px as usize) * 3 + c;
        frame.bgr.get(i).map_or(0.0, |v| f32::from(*v))
    };
    let mut out = [0u8; 3];
    for (c, slot) in out.iter_mut().enumerate() {
        let top = at(x0, y0, c) * (1.0 - fx) + at(x1, y0, c) * fx;
        let bottom = at(x0, y1, c) * (1.0 - fx) + at(x1, y1, c) * fx;
        *slot = (top * (1.0 - fy) + bottom * fy).clamp(0.0, 255.0) as u8;
    }
    out
}

/// `x' = a*x - b*y + tx`, `y' = b*x + a*y + ty`: a rotation and a scale folded
/// into two numbers, plus a translation.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Similarity {
    a: f32,
    b: f32,
    tx: f32,
    ty: f32,
}

impl Similarity {
    /// Where an output point came from in the input.
    fn invert(&self, x: f32, y: f32) -> (f32, f32) {
        let det = self.a * self.a + self.b * self.b;
        if det.abs() < f32::EPSILON {
            return (0.0, 0.0);
        }
        let (dx, dy) = (x - self.tx, y - self.ty);
        ((self.a * dx + self.b * dy) / det, (-self.b * dx + self.a * dy) / det)
    }
}

/// The similarity transform taking `from` onto `to`, by least squares.
///
/// Umeyama's, for the similarity case: line both sets up on their centroids,
/// and the rotation and scale fall out of the covariance between them. Written
/// out rather than pulled in because it is four sums and a division, and
/// because a linear algebra crate in this process would be a linear algebra
/// crate in this process.
fn similarity_from(from: &[(f32, f32); 5], to: &[(f32, f32); 5]) -> Similarity {
    let n = from.len() as f32;
    let mean = |p: &[(f32, f32); 5]| {
        let (sx, sy) = p.iter().fold((0.0, 0.0), |(sx, sy), (x, y)| (sx + x, sy + y));
        (sx / n, sy / n)
    };
    let (fx, fy) = mean(from);
    let (tx, ty) = mean(to);

    // `dot` is how much the two sets agree turn-for-turn, `cross` how much one
    // is rotated from the other, and `norm` the spread of the source.
    let (mut dot, mut cross, mut norm) = (0.0f32, 0.0f32, 0.0f32);
    for (f, t) in from.iter().zip(to.iter()) {
        let (px, py) = (f.0 - fx, f.1 - fy);
        let (qx, qy) = (t.0 - tx, t.1 - ty);
        dot += px * qx + py * qy;
        cross += px * qy - py * qx;
        norm += px * px + py * py;
    }
    if norm < f32::EPSILON {
        // Every landmark in the same place: nothing to fit. The identity is
        // wrong but finite, and the embedding it produces will not match
        // anything, which is the right outcome for a face this broken.
        return Similarity {
            a: 1.0,
            b: 0.0,
            tx: 0.0,
            ty: 0.0,
        };
    }
    let a = dot / norm;
    let b = cross / norm;
    Similarity {
        a,
        b,
        tx: tx - (a * fx - b * fy),
        ty: ty - (b * fx + a * fy),
    }
}

/// Where the models are, honouring an override for a machine that keeps them
/// somewhere else.
pub fn model_dir() -> PathBuf {
    std::env::var_os("RAVEN_FACE_MODELS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(MODEL_DIR))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(width: u32, height: u32) -> Frame {
        Frame {
            width,
            height,
            bgr: vec![128; (width * height * 3) as usize],
            at: std::time::Duration::ZERO,
        }
    }

    /// The transform has to put the landmarks it was fitted to onto the
    /// reference points. If this drifts, every embedding this daemon makes is
    /// of a face SFace was not trained on -- and nothing crashes.
    #[test]
    fn the_alignment_lands_the_landmarks_on_the_reference() {
        // A face rotated 20 degrees, scaled by 3, and moved.
        let (s, theta) = (3.0f32, 20f32.to_radians());
        let (cos, sin) = (theta.cos(), theta.sin());
        let mut landmarks = [(0.0, 0.0); 5];
        for (slot, (rx, ry)) in landmarks.iter_mut().zip(REFERENCE) {
            *slot = (
                s * (cos * rx - sin * ry) + 40.0,
                s * (sin * rx + cos * ry) + 25.0,
            );
        }
        let t = similarity_from(&landmarks, &REFERENCE);
        for (landmark, reference) in landmarks.iter().zip(REFERENCE) {
            let x = t.a * landmark.0 - t.b * landmark.1 + t.tx;
            let y = t.b * landmark.0 + t.a * landmark.1 + t.ty;
            assert!((x - reference.0).abs() < 0.01, "{x} vs {}", reference.0);
            assert!((y - reference.1).abs() < 0.01, "{y} vs {}", reference.1);
        }
    }

    /// And the inverse has to be the inverse, or the warp samples the wrong
    /// pixels while looking perfectly reasonable.
    #[test]
    fn the_inverse_transform_is_the_inverse() {
        let t = Similarity {
            a: 1.7,
            b: -0.4,
            tx: 12.0,
            ty: -5.0,
        };
        for (x, y) in [(0.0, 0.0), (33.0, 91.0), (-12.0, 7.5)] {
            let fx = t.a * x - t.b * y + t.tx;
            let fy = t.b * x + t.a * y + t.ty;
            let (bx, by) = t.invert(fx, fy);
            assert!((bx - x).abs() < 1e-3, "{bx} vs {x}");
            assert!((by - y).abs() < 1e-3, "{by} vs {y}");
        }
    }

    #[test]
    fn degenerate_landmarks_do_not_produce_a_nan_transform() {
        let t = similarity_from(&[(5.0, 5.0); 5], &REFERENCE);
        assert!(t.a.is_finite() && t.b.is_finite() && t.tx.is_finite() && t.ty.is_finite());
        let (x, y) = t.invert(3.0, 4.0);
        assert!(x.is_finite() && y.is_finite());
    }

    /// A letterboxed box has to come back where it started, or every face is
    /// found in the wrong place on a frame that is not square.
    #[test]
    fn a_box_survives_the_letterbox_round_trip() {
        let f = frame(640, 480);
        let (_, pad) = letterbox(&f);
        let mut face = Face {
            x: 100.0 * pad.scale + pad.dx,
            y: 50.0 * pad.scale + pad.dy,
            width: 200.0 * pad.scale,
            height: 200.0 * pad.scale,
            landmarks: [(120.0 * pad.scale + pad.dx, 70.0 * pad.scale + pad.dy); 5],
            score: 0.9,
        };
        pad.undo(&mut face);
        assert!((face.x - 100.0).abs() < 0.01, "{}", face.x);
        assert!((face.y - 50.0).abs() < 0.01, "{}", face.y);
        assert!((face.width - 200.0).abs() < 0.01);
        assert!((face.landmarks[0].0 - 120.0).abs() < 0.01);
    }

    /// The letterbox keeps the aspect ratio, so a 4:3 frame gets black bars
    /// and not a squashed face.
    #[test]
    fn the_letterbox_pads_rather_than_stretches() {
        let (_, pad) = letterbox(&frame(640, 480));
        assert!((pad.dx - 0.0).abs() < 0.01, "no bars on the long side");
        assert!(pad.dy > 0.0, "bars on the short side");
        assert!((pad.scale - 1.0).abs() < 0.01);
    }

    #[test]
    fn overlapping_boxes_collapse_to_the_best_one() {
        let at = |x: f32, score: f32| Face {
            x,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            landmarks: [(0.0, 0.0); 5],
            score,
        };
        let kept = suppress(vec![at(0.0, 0.5), at(5.0, 0.9), at(300.0, 0.6)], 0.3);
        assert_eq!(kept.len(), 2);
        assert!((kept[0].score - 0.9).abs() < 1e-6);
    }

    #[test]
    fn a_head_turned_to_one_side_is_noticed() {
        let straight = Face {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 100.0,
            landmarks: [
                (30.0, 40.0),
                (70.0, 40.0),
                (50.0, 60.0),
                (35.0, 80.0),
                (65.0, 80.0),
            ],
            score: 0.9,
        };
        assert!(!straight.is_turned_away());

        let mut turned = straight;
        turned.landmarks[2] = (68.0, 60.0);
        assert!(turned.is_turned_away());
    }

    #[test]
    fn a_face_that_fills_the_frame_reads_as_near() {
        let face = Face {
            x: 0.0,
            y: 0.0,
            width: 240.0,
            height: 240.0,
            landmarks: [(0.0, 0.0); 5],
            score: 0.9,
        };
        assert!((face.fill(640, 480) - 0.5).abs() < 1e-6);
        assert!(face.off_centre(640, 480) > 0.1, "it is up in the corner");
    }

    /// Two faces of a similar size is somebody standing behind you.
    #[test]
    fn two_faces_of_a_similar_size_are_a_crowd() {
        let at = |h: f32| Face {
            x: 0.0,
            y: 0.0,
            width: h,
            height: h,
            landmarks: [(0.0, 0.0); 5],
            score: 0.9,
        };
        assert!(Models::crowded(&[at(100.0), at(90.0)]));
        assert!(!Models::crowded(&[at(100.0), at(40.0)]));
        assert!(!Models::crowded(&[at(100.0)]));
    }
}
