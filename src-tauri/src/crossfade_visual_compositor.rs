//! Phase J phase 1: bounded RGB composition kernel, test-only until clock integration.
pub(crate) fn blend_rgb(
    a: &[u8],
    b: &[u8],
    width: u32,
    height: u32,
    incoming: f64,
) -> Result<Vec<u8>, String> {
    let length = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or("RGB geometry overflow")?;
    if width == 0
        || height == 0
        || width > 960
        || height > 540
        || a.len() as u64 != length
        || b.len() as u64 != length
        || !incoming.is_finite()
        || !(0.0..=1.0).contains(&incoming)
    {
        return Err("Invalid bounded RGB composition inputs".into());
    }
    Ok(a.iter()
        .zip(b)
        .map(|(&outgoing, &next)| {
            (f64::from(outgoing) * (1.0 - incoming) + f64::from(next) * incoming).round() as u8
        })
        .collect())
}

#[test]
fn dissolve_preserves_endpoints_and_blends_channels_without_mutating_inputs() {
    let a = vec![255, 0, 0];
    let b = vec![0, 0, 255];
    assert_eq!(blend_rgb(&a, &b, 1, 1, 0.0).unwrap(), a);
    assert_eq!(blend_rgb(&a, &b, 1, 1, 1.0).unwrap(), b);
    assert_eq!(blend_rgb(&a, &b, 1, 1, 0.5).unwrap(), [128, 0, 128]);
    assert_eq!(a, [255, 0, 0]);
    assert_eq!(b, [0, 0, 255]);
}

#[test]
fn malformed_geometry_and_weights_fail_before_composition() {
    for weight in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(blend_rgb(&[0; 3], &[0; 3], 1, 1, weight).is_err());
    }
    assert!(blend_rgb(&[], &[], 0, 1, 0.5).is_err());
    assert!(blend_rgb(&[0; 3], &[0; 2], 1, 1, 0.5).is_err());
    assert!(blend_rgb(&[], &[], u32::MAX, u32::MAX, 0.5).is_err());
}
