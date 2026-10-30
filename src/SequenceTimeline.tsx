import {useId,useRef,useState} from "react";
import {ArrowLeft,ArrowRight,Plus,RotateCcw,Scissors,Trash2,Undo2,Redo2} from "lucide-react";
import {NumberField} from "./EditorFields";
import {ClipEdits} from "./ClipEdits";
import {sourceEndLimit} from "./videoProject";
import {clockTime,markSeconds} from "./videoTime";
import {projectDuration,timelineEntries} from "./timelineCommands";
import {useTimelineActions} from "./useTimelineActions";
import {type SequenceSession} from "./videoSequence";

/** Placement is derived from clip order, never rounded or stored as duplicate timing. */
export function SequenceTimeline({session,disabled,assetId,onSelectAsset,selected,onSelected,position,onPosition}:{session:SequenceSession;disabled:boolean;assetId:string;onSelectAsset:(id:string)=>void;selected:string;onSelected:(id:string)=>void;position:number;onPosition:(seconds:number)=>void}){
  const project=session.history.present,entries=timelineEntries(project),duration=projectDuration(project);
  const [zoom,setZoom]=useState(1);
  const dragging=useRef("");
  const playheadId=useId();

  const playhead=Math.min(position,duration);
  function choose(id:string){onSelected(id);const item=entries.find(e=>e.clip.id===id);if(item){onSelectAsset(item.clip.asset_id);onPosition(item.start);}}
  const {command,history,error}=useTimelineActions(session,selected,onSelected);
  return <section aria-label="Project timeline" className="sequence-timeline">
    <div className="timeline-heading"><h2>Timeline</h2><p>{entries.length} {entries.length===1?"clip":"clips"} · Project duration <output aria-label="Project duration">{clockTime(duration)}</output></p></div>
    <fieldset disabled={disabled}>
      <div className="video-transport timeline-actions">
        <button className="secondary-button" disabled={!session.history.past.length} aria-label="Undo timeline edit" title="Undo timeline edit" onClick={()=>history("undo")}><Undo2 size={16}/>Undo</button>
        <button className="secondary-button" disabled={!session.history.future.length} aria-label="Redo timeline edit" title="Redo timeline edit" onClick={()=>history("redo")}><Redo2 size={16}/>Redo</button>
        <button className="secondary-button" aria-label="Append selected video" title="Add selected media to the end of the timeline" onClick={()=>command({type:"append",assetId,newId:crypto.randomUUID()})}><Plus size={16}/>Add clip</button>
        <label>Timeline zoom<select value={zoom} onChange={e=>setZoom(Number(e.target.value))}><option value={1}>Fit timeline</option><option value={2}>2×</option><option value={4}>4×</option><option value={8}>8×</option></select></label>
      </div>
      <div className="timeline-scroll" tabIndex={0} aria-label="Scrollable timeline">
        <div className="timeline-track" style={{width:`${zoom*100}%`}}>
          <div className="timeline-scrubber"><label htmlFor={playheadId}>Project playhead</label><output aria-label="Project playhead time">{clockTime(playhead)}</output><input id={playheadId} aria-label="Project playhead" type="range" min={0} max={duration||1} step="any" value={playhead} disabled={!entries.length} onChange={e=>onPosition(Number(e.target.value))}/></div>
          <ol className="timeline-clips">{entries.map((item,i)=>{
            const source=project.assets.find(a=>a.id===item.clip.asset_id)!;
            return <li key={item.clip.id} style={{flexGrow:item.duration/duration}} onDragOver={e=>{if(dragging.current&&!disabled)e.preventDefault();}} onDrop={e=>{e.preventDefault();if(dragging.current&&!disabled)command({type:"move",clipId:dragging.current,index:i});dragging.current="";}}>
              <button type="button" draggable={!disabled} className="timeline-clip" aria-label={`Clip ${i+1}: ${source.relative_path.split("/").at(-1)}`} aria-pressed={selected===item.clip.id} onClick={()=>choose(item.clip.id)} onDragStart={e=>{dragging.current=item.clip.id;e.dataTransfer.effectAllowed="move";e.dataTransfer.setData("text/plain",item.clip.id);}} onDragEnd={()=>{dragging.current="";}}>
                <strong>{i+1}. {source.relative_path.split("/").at(-1)}</strong><span>{clockTime(item.duration)}</span><small>{item.clip.edits.muted?"Audio muted":source.info.audio.length?"Linked original audio":"Silent source"}</small>
              </button>
            </li>;
          })}</ol>
          {!!duration&&<span className="timeline-playhead" aria-hidden="true" style={{left:`${playhead/duration*100}%`}}/>}
          {!entries.length&&<p className="timeline-empty">The timeline is empty. Choose a video in the media bin and append it, or undo the deletion.</p>}
        </div>
      </div>

    </fieldset>
    {error&&<p role="alert" className="field-error">{error}</p>}
    <p className="timeline-help">Drag clips to reorder, or use Move earlier / Move later. Deleting a clip closes the gap and keeps its original in the media bin. Joined clips use the project frame clock; source trim marks retain their original timing.</p>
  </section>;
}


/** Selected-clip controls share the same session/commands as the persistent timeline. */
export function SequenceClipInspector({session,disabled,selected,onSelected,position}:{session:SequenceSession;disabled:boolean;selected:string;onSelected:(id:string)=>void;position:number}){
  const project=session.history.present,entries=timelineEntries(project),entry=entries.find(e=>e.clip.id===selected);
  const clip=entry?.clip,asset=project.assets.find(a=>a.id===clip?.asset_id),index=entries.findIndex(e=>e.clip.id===selected),playhead=Math.min(position,projectDuration(project));
  const {command,error}=useTimelineActions(session,selected,onSelected);
  return <section aria-label="Selected clip controls"><h2>Selected clip</h2><fieldset disabled={disabled}>
      {clip&&asset&&entry&&<div className="timeline-inspector" key={clip.id}>
        <p><strong>Selected clip {index+1}</strong> · Source duration {clockTime(asset.info.duration_seconds)} · Project position {clockTime(entry.start)}–{clockTime(entry.end)}</p>
        <div className="timeline-trim-fields">
          <NumberField label="Clip start in source (seconds)" value={markSeconds(clip.source_in)} min={0} max={sourceEndLimit(asset.info)} step="any" onCommit={start=>command({type:"trim",clipId:clip.id,start,end:markSeconds(clip.source_out)})}/>
          <NumberField label="Clip end in source (seconds)" value={markSeconds(clip.source_out)} min={0} max={sourceEndLimit(asset.info)} step="any" onCommit={end=>command({type:"trim",clipId:clip.id,start:markSeconds(clip.source_in),end})}/>
        </div>
        <div className="video-transport timeline-clip-actions">
          <button className="secondary-button" disabled={index===0} onClick={()=>command({type:"move",clipId:clip.id,index:index-1})}><ArrowLeft size={16}/>Move earlier</button>
          <button className="secondary-button" disabled={index===entries.length-1} onClick={()=>command({type:"move",clipId:clip.id,index:index+1})}>Move later<ArrowRight size={16}/></button>
          <button className="secondary-button" disabled={playhead<=entry.start||playhead>=entry.end} onClick={()=>command({type:"split",clipId:clip.id,at:playhead,newId:crypto.randomUUID()})}><Scissors size={16}/>Split at playhead</button>
          <button className="destructive-button" onClick={()=>command({type:"delete",clipId:clip.id})}><Trash2 size={16}/>Ripple delete clip</button>
          <button className="secondary-button" onClick={()=>command({type:"reset",clipId:clip.id})}><RotateCcw size={16}/>Reset selected clip</button>
        </div>
        <ClipEdits edits={clip.edits} info={asset.info} onChange={mutate=>{const edits=structuredClone(clip.edits);mutate(edits);return command({type:"edits",clipId:clip.id,edits});}}/>
      </div>}
    {!clip&&<p>Select a clip on the timeline to edit it.</p>}
    </fieldset>{error&&<p role="alert" className="field-error">{error}</p>}</section>;
}
