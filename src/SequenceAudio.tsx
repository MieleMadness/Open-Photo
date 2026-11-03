import {useState} from "react";
import {type SequenceSession,type AudioTrack} from "./videoSequence";
import {projectDuration} from "./timelineCommands";

export function SequenceAudio({session,disabled,playhead,onImport}:{session:SequenceSession;disabled:boolean;playhead:number;onImport:()=>void}){
  const project=session.history.present;
  const tracks=project.audio_tracks||[];
  const duration=projectDuration(project);
  const videos=project.assets.filter(asset=>asset.media.kind==="video"&&asset.media.info.audio.length>0);
  const sources=[...videos.map(asset=>({id:asset.id,label:asset.relative_path,duration:asset.media.kind==="video"?asset.duration:0})),...(project.audio_sources||[]).map(asset=>({id:asset.id,label:asset.relative_path,duration:asset.info.duration_seconds}))];
  const [chosenSource,setChosenSource]=useState("");
  function commit(next:AudioTrack[]){session.commit({...project,audio_tracks:next});}
  function update(id:string,changes:Partial<AudioTrack>){commit(tracks.map(track=>track.id===id?{...track,...changes}:track));}
  return <section aria-label="Audio tracks" className="sequence-titles">
    <div className="timeline-heading"><h2>Audio tracks</h2><p>{tracks.length} / 8</p></div>
    <p className="timeline-help">Layer sound from imported videos or standalone audio. Audio plays in rendered previews and exports.</p>
    <button className="secondary-button" type="button" disabled={disabled||(project.audio_sources||[]).length>=8} onClick={onImport}>Import audio files…</button>
    {tracks.length>0&&duration>0&&<div className="sequence-audio-overview" role="group" aria-label="Audio track placement on project timeline">
      <div className="sequence-audio-ruler"><span>0:00</span><span>{duration.toFixed(1)} s</span></div>
      {tracks.map((track,index)=><div key={track.id} className="sequence-audio-lane" aria-label={`Audio track ${index+1}: starts at ${track.start_seconds.toFixed(1)} seconds, lasts ${track.duration_seconds.toFixed(1)} seconds`}>
        <span className="sequence-audio-bar" style={{left:`${track.start_seconds/duration*100}%`,width:`${track.duration_seconds/duration*100}%`}} title={`Track ${index+1}: ${track.start_seconds.toFixed(2)}–${(track.start_seconds+track.duration_seconds).toFixed(2)} seconds`}>{index+1}</span>
        <span className="sequence-audio-playhead" aria-hidden="true" style={{left:`${Math.min(100,Math.max(0,playhead/duration*100))}%`}}/>
      </div>)}
      <p className="sequence-audio-hint">Track bars show placement; use the controls below to edit timing. The vertical marker follows your playhead.</p>
    </div>}
    <fieldset disabled={disabled||duration<=0}>
      {sources.length>0&&<label className="sequence-audio-source-picker">Audio source<select aria-label="Choose video sound for new track" value={sources.some(video=>video.id===chosenSource)?chosenSource:sources[0].id} onChange={event=>setChosenSource(event.target.value)}>{sources.map(video=><option key={video.id} value={video.id}>{video.relative_path}</option>)}</select></label>}
      <button className="secondary-button" type="button" disabled={tracks.length>=8||sources.length===0} onClick={()=>{
        const asset=sources.find(video=>video.id===chosenSource)||sources[0];
        const start=Math.min(Math.max(0,playhead),Math.max(0,duration-.04));
        const length=Math.min(3,asset.duration,duration-start);
        if(length<.04)return;
        commit([...tracks,{id:crypto.randomUUID(),asset_id:asset.id,start_seconds:start,source_in_seconds:0,duration_seconds:length,gain_percent:100,fade_in_seconds:0,fade_out_seconds:0}]);
      }}>Add audio from video</button>
      {tracks.map(track=>{
        const asset=sources.find(a=>a.id===track.asset_id);
        return <div key={track.id} className="sequence-title-fields">
          <label>Audio source<select value={track.asset_id} aria-label="Audio source" onChange={event=>{
            const next=sources.find(a=>a.id===event.target.value);if(!next)return;
            const nextDuration=Math.min(track.duration_seconds,next.duration);
            const fadeIn=Math.min(track.fade_in_seconds,nextDuration);
            update(track.id,{asset_id:next.id,source_in_seconds:0,duration_seconds:nextDuration,fade_in_seconds:fadeIn,fade_out_seconds:Math.min(track.fade_out_seconds,nextDuration-fadeIn)});
          }}>{sources.map(a=><option key={a.id} value={a.id}>{a.relative_path}</option>)}</select></label>
          <label>Start (seconds)<input aria-label="Audio start" type="number" min={0} max={Math.max(0,duration-track.duration_seconds)} step=".01" value={track.start_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&n+track.duration_seconds<=duration)update(track.id,{start_seconds:n});}}/></label>
          <label>Source offset (seconds)<input aria-label="Audio source offset" type="number" min={0} max={Math.max(0,(asset?.duration||0)-track.duration_seconds)} step=".01" value={track.source_in_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&!!asset&&n+track.duration_seconds<=asset.media.info.duration_seconds)update(track.id,{source_in_seconds:n});}}/></label>
          <label>Duration (seconds)<input aria-label="Audio duration" type="number" min=".04" max={Math.min(duration-track.start_seconds,(asset?.media.kind==="video"?asset.media.info.duration_seconds:0)-track.source_in_seconds)} step=".01" value={track.duration_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=.04&&n<=duration-track.start_seconds&&asset?.media.kind==="video"&&track.source_in_seconds+n<=asset.media.info.duration_seconds&&n>=track.fade_in_seconds+track.fade_out_seconds)update(track.id,{duration_seconds:n});}}/></label>
          <label>Volume (%)<input aria-label="Audio volume" type="number" min={0} max={200} step={5} value={track.gain_percent} onChange={event=>{const n=Number(event.target.value);if(Number.isInteger(n)&&n>=0&&n<=200)update(track.id,{gain_percent:n});}}/></label>
          <label>Fade in (seconds)<input aria-label="Audio fade in" type="number" min={0} max={track.duration_seconds-track.fade_out_seconds} step=".01" value={track.fade_in_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&n+track.fade_out_seconds<=track.duration_seconds)update(track.id,{fade_in_seconds:n});}}/></label>
          <label>Fade out (seconds)<input aria-label="Audio fade out" type="number" min={0} max={track.duration_seconds-track.fade_in_seconds} step=".01" value={track.fade_out_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&n+track.fade_in_seconds<=track.duration_seconds)update(track.id,{fade_out_seconds:n});}}/></label>
          <button type="button" className="secondary-button" onClick={()=>commit(tracks.filter(item=>item.id!==track.id))}>Remove audio track</button>
        </div>;
      })}
    </fieldset>
  </section>;
}
