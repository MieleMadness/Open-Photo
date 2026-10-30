import {useEffect,useId,useRef,useState} from "react";
import {ArrowLeft,ArrowRight,Plus,RotateCcw,Scissors,Trash2,Undo2,Redo2,ZoomIn,ZoomOut,Maximize2} from "lucide-react";
import {TimelineTrimHandles} from "./TimelineTrimHandles";
import {EditorPoster} from "./EditorPoster";
import {timelineTicks,clipDropIndex} from "./timelineGeometry";
import type {LoadedSequence} from "./videoSequence";
import {NumberField} from "./EditorFields";
import {assetGeometry,hasAudio,videoInfo} from "./sequenceMedia";
import {ClipEdits} from "./ClipEdits";
import {clockTime,markSeconds} from "./videoTime";
import {projectDuration,timelineEntries} from "./timelineCommands";
import {useTimelineActions} from "./useTimelineActions";
import {type SequenceSession} from "./videoSequence";

/** Placement is derived from clip order, never rounded or stored as duplicate timing. */
export function SequenceTimeline({session,disabled,assetId,onSelectAsset,selected,onSelected,position,onPosition,sources=[]}:{sources?:LoadedSequence["sources"];session:SequenceSession;disabled:boolean;assetId:string;onSelectAsset:(id:string)=>void;selected:string;onSelected:(id:string)=>void;position:number;onPosition:(seconds:number)=>void}){
  const project=session.history.present,entries=timelineEntries(project),duration=projectDuration(project);
  const [zoom,setZoom]=useState(1);
  const dragging=useRef("");
  const viewport=useRef<HTMLDivElement>(null),track=useRef<HTMLDivElement>(null);
  const [width,setWidth]=useState(800),[drop,setDrop]=useState<{id:string;after:boolean}|null>(null);
  useEffect(()=>{
    const measure=()=>{if(viewport.current?.clientWidth)setWidth(viewport.current.clientWidth);};measure();
    const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(measure):null;
    if(viewport.current)observer?.observe(viewport.current);
    return()=>observer?.disconnect();
  },[]);
  const ticks=timelineTicks(duration,width*zoom);
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
        <button className="secondary-button" aria-label="Append selected media" title="Add selected media to the end of the timeline" onClick={()=>command({type:"append",assetId,newId:crypto.randomUUID()})}><Plus size={16}/>Add clip</button>
        <div className="timeline-zoom" role="group" aria-label="Timeline zoom">
          <button type="button" className="icon-button" aria-label="Zoom timeline out" title="Zoom timeline out" disabled={zoom<=1} onClick={()=>setZoom(value=>Math.max(1,value/2))}><ZoomOut size={16}/></button>
          <button type="button" className="secondary-button" aria-label="Fit timeline" title="Fit the whole project in the timeline" onClick={()=>setZoom(1)}><Maximize2 size={15}/>Fit</button>
          <button type="button" className="icon-button" aria-label="Zoom timeline in" title="Zoom timeline in" disabled={zoom>=8} onClick={()=>setZoom(value=>Math.min(8,value*2))}><ZoomIn size={16}/></button>
          <output aria-label="Timeline zoom level">{zoom}×</output>
        </div>
      </div>
      <div ref={viewport} className="timeline-scroll" tabIndex={0} aria-label="Scrollable timeline">
        <div ref={track} className="timeline-track" style={{width:`${zoom*100}%`}}>
          <div className="timeline-scrubber"><label htmlFor={playheadId}>Project playhead</label><output aria-label="Project playhead time">{clockTime(playhead)}</output><div className="timeline-ruler"><div className="timeline-ticks" aria-hidden="true">{ticks.map(tick=><span key={tick.seconds} data-end={tick.left>95} style={{left:`${tick.left}%`}}>{tick.label}</span>)}</div><input id={playheadId} aria-label="Project playhead" title="Drag the ruler or use arrow keys to seek" type="range" min={0} max={duration||1} step="any" value={playhead} disabled={!entries.length} onChange={e=>onPosition(Number(e.target.value))}/><span className="timeline-ruler-head" aria-hidden="true" style={{left:`${duration?playhead/duration*100:0}%`}}/></div></div>
          <ol className="timeline-clips">{entries.map((item,i)=>{
            const source=project.assets.find(a=>a.id===item.clip.asset_id)!;
            const binding=sources.find(value=>value.asset_id===source.id);
            const [tickNumerator,tickDenominator]=item.clip.source_in.time_base.split("/").map(Number),sourceTick=tickNumerator/tickDenominator;
            return <li key={item.clip.id} style={{flexGrow:item.duration/duration}} data-drop={drop?.id===item.clip.id?(drop.after?"after":"before"):undefined} onDragOver={e=>{
              if(!dragging.current||disabled)return;e.preventDefault();
              const bounds=e.currentTarget.getBoundingClientRect(),after=e.clientX>=bounds.left+bounds.width/2;
              const from=entries.findIndex(value=>value.clip.id===dragging.current),index=clipDropIndex(from,i,after);
              setDrop(previous=>index===from?null:previous?.id===item.clip.id&&previous.after===after?previous:{id:item.clip.id,after});
            }} onDrop={e=>{
              e.preventDefault();if(dragging.current&&!disabled){
                const from=entries.findIndex(value=>value.clip.id===dragging.current),bounds=e.currentTarget.getBoundingClientRect();
                const index=clipDropIndex(from,i,e.clientX>=bounds.left+bounds.width/2);
                if(from>=0&&index!==from)command({type:"move",clipId:dragging.current,index});
              }
              dragging.current="";setDrop(null);
            }}>
              <button type="button" draggable={!disabled} className="timeline-clip" aria-label={`Clip ${i+1}: ${source.relative_path.split("/").at(-1)}`} aria-pressed={selected===item.clip.id} onClick={()=>choose(item.clip.id)} onDragStart={e=>{dragging.current=item.clip.id;e.dataTransfer.effectAllowed="move";e.dataTransfer.setData("text/plain",item.clip.id);}} onDragEnd={()=>{dragging.current="";setDrop(null);}}>
                <strong title={source.relative_path}>{i+1}. {source.relative_path.split("/").at(-1)}</strong><EditorPoster root={binding?.root||undefined} path={binding?.path||undefined} identity={source.sha256} photo={source.media.kind==="photo"} filmstrip={source.media.kind==="video"}/><span className="timeline-clip-duration">{clockTime(item.duration)}</span><small className="timeline-linked-audio" data-muted={item.clip.edits.muted||!hasAudio(source)}>{source.media.kind==="photo"?"Still image":item.clip.edits.muted?"Audio muted":hasAudio(source)?"Linked original audio":"Silent source"}</small>
              </button>
              <TimelineTrimHandles range={{start:markSeconds(item.clip.source_in),end:markSeconds(item.clip.source_out)}} limit={Math.max((videoInfo(source)?.duration_seconds||600),markSeconds(item.clip.source_out))} minimum={0.04+sourceTick} step={Math.max(1/project.frame_rate,sourceTick)} selected={selected===item.clip.id} disabled={disabled} token={project} index={i} pixelsPerSecond={()=>(track.current?.getBoundingClientRect().width||0)/duration} onCommit={range=>command({type:"trim",clipId:item.clip.id,...range})}/>
            </li>;
          })}</ol>
          {!!duration&&<span className="timeline-playhead" aria-hidden="true" style={{left:`${playhead/duration*100}%`}}/>}
          {!entries.length&&<p className="timeline-empty">The timeline is empty. Choose media in the media bin and append it, or undo the deletion.</p>}
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
        <p><strong>Selected clip {index+1}</strong> · {asset.media.kind==="photo"?"Still image":`Source duration ${clockTime(videoInfo(asset)!.duration_seconds)}`} · Project position {clockTime(entry.start)}–{clockTime(entry.end)}</p>
        {asset.media.kind==="photo"&&<NumberField label="Photo duration (seconds)" value={markSeconds(clip.source_out)-markSeconds(clip.source_in)} min={.04} max={600-markSeconds(clip.source_in)} step="any" onCommit={duration=>command({type:"trim",clipId:clip.id,start:markSeconds(clip.source_in),end:markSeconds(clip.source_in)+duration})}/>}
        <div className="video-transport timeline-clip-actions">
          <button className="secondary-button" disabled={index===0} onClick={()=>command({type:"move",clipId:clip.id,index:index-1})}><ArrowLeft size={16}/>Move earlier</button>
          <button className="secondary-button" disabled={index===entries.length-1} onClick={()=>command({type:"move",clipId:clip.id,index:index+1})}>Move later<ArrowRight size={16}/></button>
          <button className="secondary-button" disabled={playhead<=entry.start||playhead>=entry.end} onClick={()=>command({type:"split",clipId:clip.id,at:playhead,newId:crypto.randomUUID()})}><Scissors size={16}/>Split at playhead</button>
          <button className="destructive-button" onClick={()=>command({type:"delete",clipId:clip.id})}><Trash2 size={16}/>Ripple delete clip</button>
          <button className="secondary-button" onClick={()=>command({type:"reset",clipId:clip.id})}><RotateCcw size={16}/>Reset selected clip</button>
        </div>
        <ClipEdits edits={clip.edits} info={assetGeometry(asset)} photo={asset.media.kind==="photo"} onChange={mutate=>{const edits=structuredClone(clip.edits);mutate(edits);return command({type:"edits",clipId:clip.id,edits});}}/>
      </div>}
    {!clip&&<p>Select a clip on the timeline to edit it.</p>}
    </fieldset>{error&&<p role="alert" className="field-error">{error}</p>}</section>;
}
