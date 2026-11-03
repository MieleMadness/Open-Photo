//! Export resolution policy is independent of timeline edits and the project canvas.
use crate::video_project::Output;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExportSize {
    #[default]
    Project,
    Hd1080,
    Uhd4k,
}

/// Cap resolution inside a landscape/portrait envelope; never enlarge the project canvas.
pub fn resolve(canvas: &Output, preset: ExportSize) -> Output {
    if preset == ExportSize::Project {
        return canvas.clone();
    }
    let (mut w, mut h) = if preset == ExportSize::Hd1080 {
        (1920.0, 1080.0)
    } else {
        (3840.0, 2160.0)
    };
    if canvas.height > canvas.width {
        std::mem::swap(&mut w, &mut h);
    }
    let factor = (w / f64::from(canvas.width))
        .min(h / f64::from(canvas.height))
        .min(1.0);
    Output {
        width: ((f64::from(canvas.width) * factor / 2.0).floor() as u32 * 2).max(16),
        height: ((f64::from(canvas.height) * factor / 2.0).floor() as u32 * 2).max(16),
        fit: canvas.fit.clone(),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presets_preserve_portrait_square_ultrawide_and_smaller_canvases() {
        for (w, h, preset, expected) in [
            (3840, 2160, ExportSize::Hd1080, (1920, 1080)),
            (2160, 3840, ExportSize::Hd1080, (1080, 1920)),
            (3840, 1080, ExportSize::Hd1080, (1920, 540)),
            (2160, 2160, ExportSize::Hd1080, (1080, 1080)),
            (1280, 720, ExportSize::Uhd4k, (1280, 720)),
            (3840, 2160, ExportSize::Uhd4k, (3840, 2160)),
            (2160, 3840, ExportSize::Uhd4k, (2160, 3840)),
        ] {
            let canvas = Output {
                width: w,
                height: h,
                fit: "fit".into(),
            };
            let original = canvas.clone();
            let output = resolve(&canvas, preset);
            assert_eq!((output.width, output.height), expected);
            assert_eq!(canvas, original);
        }
    }
}
