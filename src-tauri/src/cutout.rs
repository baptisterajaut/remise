//! `remise --cutout <onnxruntime lib> <model> <in.rgb> <out.gray>`: the background-removal mask,
//! in a child process so that ONNX Runtime (loaded from the bundled library, never linked) can't
//! take the app down with it. ImageMagick does the decoding and the compositing around it.
//!
//! The model is BiRefNet (general, lite): a 1024×1024 RGB image in, a 1024×1024 mask out.
//! Pre/post-processing as in rembg (MIT): pixels over their maximum, ImageNet mean and deviation,
//! a sigmoid on the logits, then stretched to 0–255.

use ort::session::Session;
use ort::value::Tensor;
use std::fs;

pub const SIDE: usize = 1024;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

pub fn main(args: &[String]) -> Result<(), String> {
    let [lib, model, rgb, mask] = args else {
        return Err("usage: remise --cutout <onnxruntime library> <model.onnx> <in.rgb> <out.gray>".into());
    };
    #[cfg(windows)]
    preload_crt(std::path::Path::new(lib));
    let env = ort::init_from(lib).map_err(|e| format!("can't load ONNX Runtime: {e}"))?.build().map_err(|e| e.to_string())?;
    let mut session =
        Session::builder(&env).and_then(|mut b| b.commit_from_file(model)).map_err(|e| format!("can't load the model: {e}"))?;
    let px = fs::read(rgb).map_err(|e| e.to_string())?;
    let input = Tensor::from_array(([1usize, 3, SIDE, SIDE], normalize(&px)?)).map_err(|e| e.to_string())?;
    let outputs = session.run(ort::inputs![input]).map_err(|e| e.to_string())?;
    let (_, logits) = outputs[0].try_extract_tensor::<f32>().map_err(|e| e.to_string())?;
    fs::write(mask, to_mask(&logits[..SIDE * SIDE])).map_err(|e| e.to_string())
}

/// Windows looks for a DLL's dependencies in System32 before the PATH, so an old VC++ runtime
/// installed there would win over the one bundled next to ONNX Runtime (and old msvcp140 +
/// new code is a known crash). Loading ours by full path first makes them the ones in use.
#[cfg(windows)]
fn preload_crt(lib: &std::path::Path) {
    let Some(dir) = lib.parent() else { return };
    for dll in ["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll", "msvcp140_1.dll"] {
        let path = dir.join(dll);
        // SAFETY: Microsoft's own runtime DLLs, whose initialisers have no preconditions.
        if let Ok(loaded) = unsafe { libloading::Library::new(&path) } {
            std::mem::forget(loaded); // stays loaded for the life of this short process
        }
    }
}

/// Interleaved 8-bit RGB → planar, normalised f32 (NCHW).
pub fn normalize(px: &[u8]) -> Result<Vec<f32>, String> {
    let n = SIDE * SIDE;
    if px.len() != n * 3 {
        return Err(format!("expected {} bytes of RGB, got {}", n * 3, px.len()));
    }
    let max = f32::from(px.iter().copied().max().unwrap_or(1)).max(1.0);
    let mut out = vec![0.0; n * 3];
    for (i, rgb) in px.as_chunks::<3>().0.iter().enumerate() {
        for c in 0..3 {
            out[c * n + i] = (f32::from(rgb[c]) / max - MEAN[c]) / STD[c];
        }
    }
    Ok(out)
}

/// Logits → sigmoid → stretched to the full 0–255 range.
pub fn to_mask(logits: &[f32]) -> Vec<u8> {
    let p: Vec<f32> = logits.iter().map(|x| 1.0 / (1.0 + (-x).exp())).collect();
    let (lo, hi) = p.iter().fold((f32::MAX, f32::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let span = (hi - lo).max(f32::EPSILON);
    p.iter().map(|v| ((v - lo) / span * 255.0).round() as u8).collect()
}
