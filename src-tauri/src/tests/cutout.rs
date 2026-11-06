//! Background removal: the pre/post-processing around the model, and the plan around it.

use crate::cutout::{SIDE, normalize, to_mask};
use crate::tools::{self, Job, Step};
use std::collections::HashMap;
use std::path::PathBuf;

#[test]
fn pixels_are_planar_and_normalised_like_rembg() {
    assert!(normalize(&[0; 12]).is_err(), "only a 1024² RGB input");
    let mut px = vec![0u8; SIDE * SIDE * 3];
    px[..3].copy_from_slice(&[255, 0, 0]);
    let v = normalize(&px).unwrap();
    let n = SIDE * SIDE;
    assert!((v[0] - (1.0 - 0.485) / 0.229).abs() < 1e-5, "red of the first pixel, over the max");
    assert!((v[n] - (0.0 - 0.456) / 0.224).abs() < 1e-5, "green plane starts at n");
}

#[test]
fn logits_become_a_full_range_mask() {
    assert_eq!(to_mask(&[-10.0, 0.0, 10.0]), [0, 128, 255]);
    assert_eq!(to_mask(&[3.0, 3.0]), [0, 0], "a flat output doesn't divide by zero");
}

#[test]
fn remove_bg_runs_remise_as_its_own_helper() {
    let t = tools::find("img-remove-bg").unwrap();
    let j = Job {
        inputs: vec![PathBuf::from("/d/cat.heic")],
        out: PathBuf::from("/d/cat.cutout.png"),
        tmp: PathBuf::from("/tmp/job"),
        params: tools::with_defaults(t, HashMap::new()),
    };
    let steps = (t.build)(&j);
    let Step::Run("remise", a) = &steps[2] else { panic!("{steps:?}") };
    assert_eq!(a[..3], ["--cutout", "{which:onnxruntime}", "{which:bg-model}"]);
    assert!(matches!(steps[3], Step::Then(_)));
}
