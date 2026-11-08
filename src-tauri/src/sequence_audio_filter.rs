// The final limiter runs once after summation, never independently on each layer.
const FINAL_PCM_CLIP: &str = "aeval='clip(val(0),-1,1)|clip(val(1),-1,1)'";
fn added_audio_filter(
    input: usize,
    stream: usize,
    plan: &crate::crossfade_layer_clock::AudioPlan,
    label: &str,
) -> Result<String, String> {
    let count = plan
        .samples
        .end
        .checked_sub(plan.samples.start)
        .filter(|n| *n > 0)
        .ok_or("Empty audio track span")?;
    if plan
        .source_samples
        .end
        .checked_sub(plan.source_samples.start)
        != Some(count)
        || plan
            .fade_in_samples
            .checked_add(plan.fade_out_samples)
            .is_none_or(|n| n > count)
        || plan.gain_percent > 200
    {
        return Err("Invalid quantized audio track".into());
    }
    let mut gain = format!("({}/100)", plan.gain_percent);
    if plan.fade_in_samples > 0 {
        gain.push_str(&format!("*min(1,max(0,(n+0.5)/{}))", plan.fade_in_samples));
    }
    if plan.fade_out_samples > 0 {
        gain.push_str(&format!(
            "*min(1,max(0,({count}-n-0.5)/{}))",
            plan.fade_out_samples
        ));
    }
    Ok(format!("[{input}:{stream}]aresample=48000:async=1:first_pts=0,aformat=sample_fmts=flt:channel_layouts=stereo,atrim=start_sample={}:end_sample={},asetpts=PTS-STARTPTS,apad=whole_len={count},atrim=end_sample={count},aeval='val(0)*({gain})|val(1)*({gain})',adelay={}S:all=1[{label}]",plan.source_samples.start,plan.source_samples.end,plan.samples.start))
}
