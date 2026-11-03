import {useState} from "react";
import {type SequenceSession,type AudioTrack} from "./videoSequence";
import {projectDuration} from "./timelineCommands";
import {AudioWaveform} from "./AudioWaveform";
import {AudioTrimHandles} from "./AudioTrimHandles";
import {placeAudioAtPlayhead,trimAudioEndAtPlayhead} from "./audioTimeline";

export function SequenceAudio({session,disabled,playhead,onImport,onRelink,sourceStatuses,onSeek}:{session:SequenceSession;disabled:boolean;playhead:number;onImport:()=>void;onRelink:(id:string)=>void;onSeek:(seconds:number)=>void;sourceStatuses:{asset_id:string;path:string|null;status:string}[]}){
  const project=session.history.present;
  const tracks=project.audio_tracks||[];
  const duration=projectDuration(project);
  const videos=project.assets.filter(asset=>asset.media.kind==="video"&&asset.media.info.audio.length>0);
  const sources=[...videos.map(asset=>({id:asset.id,label:asset.relative_path,duration:asset.media.kind==="video"?asset.media.info.duration_seconds:0})),...(project.audio_sources||[]).map(asset=>({id:asset.id,label:asset.relative_path,duration:asset.info.duration_seconds}))];
  const [chosenSource,setChosenSource]=useState("");
  function commit(next:AudioTrack[]){session.commit({...project,audio_tracks:next});}
  function update(id:string,changes:Partial<AudioTrack>){commit(tracks.map(track=>track.id===id?{...track,...changes}:track));}
  return <section aria-label="Audio tracks" className="sequence-titles">
    <div className="timeline-heading"><h2>Audio tracks</h2><p>{tracks.length} / 8</p></div>
    <p className="timeline-help">Layer sound from imported videos or standalone audio. Audio plays in rendered previews and exports.</p>
    <button className="secondary-button" type="button" disabled={disabled||(project.audio_sources||[]).length>=8} onClick={onImport}>Import audio files…</button>
    {(project.audio_sources||[]).length>0&&<div className="sequence-audio-imports" aria-label="Imported audio files">
      {(project.audio_sources||[]).map(audio=>{
        const status=sourceStatuses.find(item=>item.asset_id===audio.id);
        return <div key={audio.id} className="sequence-audio-import">
          <span title={audio.relative_path}><strong>{audio.relative_path.split("/").at(-1)}</strong><small>{status?.path?"Ready":status?.status||"Needs relinking"} · {audio.info.duration_seconds.toFixed(1)} s</small></span>
          <button className="secondary-button" type="button" disabled={disabled} onClick={()=>onRelink(audio.id)}>Relink…</button>
        </div>;
      })}
    </div>}
    {tracks.length>0&&duration>0&&<div className="sequence-audio-overview" role="group" aria-label="Audio track placement on project timeline">
      <div className="sequence-audio-ruler"><span>0:00</span><span>{duration.toFixed(1)} s</span></div>
      {tracks.map((track,index)=><div key={track.id} className="sequence-audio-lane" aria-label={`Audio track ${index+1}: starts at ${track.start_seconds.toFixed(1)} seconds, lasts ${track.duration_seconds.toFixed(1)} seconds`}>
        {sources.find(source=>source.id===track.asset_id)?<AudioTrimHandles track={track} trackNumber={index+1} sourceDuration={sources.find(source=>source.id===track.asset_id)!.duration} projectDuration={duration} disabled={disabled} onCommit={next=>update(track.id,next)}/>:<span className="sequence-audio-bar" style={{left:`${track.start_seconds/duration*100}%`,width:`${track.duration_seconds/duration*100}%`}}>{index+1}</span>}
        <span className="sequence-audio-playhead" aria-hidden="true" style={{left:`${Math.min(100,Math.max(0,playhead/duration*100))}%`}}/>
      </div>)}
      <p className="sequence-audio-hint">Drag the audio bar edges to trim without changing originals; use arrow keys for precise nudges. The playhead actions below can also move, trim, or preview a track.</p>
    </div>}
    <fieldset disabled={disabled||duration<=0}>
      {sources.length>0&&<label className="sequence-audio-source-picker">Audio source<select aria-label="Choose audio source for new track" value={sources.some(video=>video.id===chosenSource)?chosenSource:sources[0].id} onChange={event=>setChosenSource(event.target.value)}>{sources.map(video=><option key={video.id} value={video.id}>{video.label}</option>)}</select></label>}
      <button className="secondary-button" type="button" disabled={tracks.length>=8||sources.length===0} onClick={()=>{
        const asset=sources.find(video=>video.id===chosenSource)||sources[0];
        const start=Math.min(Math.max(0,playhead),Math.max(0,duration-.04));
        const length=Math.min(3,asset.duration,duration-start);
        if(length<.04)return;
        commit([...tracks,{id:crypto.randomUUID(),asset_id:asset.id,start_seconds:start,source_in_seconds:0,duration_seconds:length,gain_percent:100,fade_in_seconds:0,fade_out_seconds:0}]);
      }}>Add audio track</button>
      {tracks.map(track=>{
        const asset=sources.find(a=>a.id===track.asset_id);
        return <div key={track.id} className="sequence-title-fields">
          <label>Audio source<select value={track.asset_id} aria-label="Audio source" onChange={event=>{
            const next=sources.find(a=>a.id===event.target.value);if(!next)return;
            const nextDuration=Math.min(track.duration_seconds,next.duration);
            const fadeIn=Math.min(track.fade_in_seconds,nextDuration);
            update(track.id,{asset_id:next.id,source_in_seconds:0,duration_seconds:nextDuration,fade_in_seconds:fadeIn,fade_out_seconds:Math.min(track.fade_out_seconds,nextDuration-fadeIn)});
          }}>{sources.map(a=><option key={a.id} value={a.id}>{a.label}</option>)}</select></label>
          {(project.audio_sources||[]).some(item=>item.id===track.asset_id)&&session.workspaceId&&<AudioWaveform
            workspaceId={session.workspaceId}
            projectId={project.id}
            assetId={track.asset_id}
            revision={session.revision}
            sourceIn={track.source_in_seconds}
            duration={track.duration_seconds}
            trackStart={track.start_seconds}
            playhead={playhead}
            onSeek={onSeek}
            ready={!!sourceStatuses.find(item=>item.asset_id===track.asset_id)?.path}
          />}
          <label>Start (seconds)<input aria-label="Audio start" type="number" min={0} max={Math.max(0,duration-track.duration_seconds)} step=".01" value={track.start_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&n+track.duration_seconds<=duration)update(track.id,{start_seconds:n});}}/></label>
          <label>Source offset (seconds)<input aria-label="Audio source offset" type="number" min={0} max={Math.max(0,(asset?.duration||0)-track.duration_seconds)} step=".01" value={track.source_in_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&!!asset&&n+track.duration_seconds<=asset.duration)update(track.id,{source_in_seconds:n});}}/></label>
          <label>Duration (seconds)<input aria-label="Audio duration" type="number" min=".04" max={Math.min(duration-track.start_seconds,(asset?.duration||0)-track.source_in_seconds)} step=".01" value={track.duration_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=.04&&n<=duration-track.start_seconds&&!!asset&&track.source_in_seconds+n<=asset.duration&&n>=track.fade_in_seconds+track.fade_out_seconds)update(track.id,{duration_seconds:n});}}/></label>
          <div className="sequence-audio-playhead-actions" role="group" aria-label="Audio track playhead actions">
            <button className="secondary-button" type="button" disabled={!placeAudioAtPlayhead(track,playhead,duration)} onClick={()=>{const next=placeAudioAtPlayhead(track,playhead,duration);if(next)update(track.id,next);}}>Move to playhead</button>
            <button className="secondary-button" type="button" disabled={!asset||!trimAudioEndAtPlayhead(track,playhead,asset.duration,duration)} onClick={()=>{const next=asset&&trimAudioEndAtPlayhead(track,playhead,asset.duration,duration);if(next)update(track.id,next);}}>End at playhead</button>
            <button className="secondary-button" type="button" onClick={()=>onSeek(Math.max(0,track.start_seconds+track.duration_seconds-0.05))}>Preview end</button>
          </div>
          <label>Volume (%)<input aria-label="Audio volume" type="number" min={0} max={200} step={5} value={track.gain_percent} onChange={event=>{const n=Number(event.target.value);if(Number.isInteger(n)&&n>=0&&n<=200)update(track.id,{gain_percent:n});}}/></label>
          <label>Fade in (seconds)<input aria-label="Audio fade in" type="number" min={0} max={track.duration_seconds-track.fade_out_seconds} step=".01" value={track.fade_in_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&n+track.fade_out_seconds<=track.duration_seconds)update(track.id,{fade_in_seconds:n});}}/></label>
          <label>Fade out (seconds)<input aria-label="Audio fade out" type="number" min={0} max={track.duration_seconds-track.fade_in_seconds} step=".01" value={track.fade_out_seconds} onChange={event=>{const n=Number(event.target.value);if(Number.isFinite(n)&&n>=0&&n+track.fade_in_seconds<=track.duration_seconds)update(track.id,{fade_out_seconds:n});}}/></label>
          <button type="button" className="secondary-button" onClick={()=>commit(tracks.filter(item=>item.id!==track.id))}>Remove audio track</button>
        </div>;
      })}
    </fieldset>
  </section>;
}
