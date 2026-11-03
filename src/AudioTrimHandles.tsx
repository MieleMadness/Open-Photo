import {useEffect,useRef,useState,type PointerEvent} from "react";
import type {AudioTrack} from "./videoSequence";
import {trimAudioEdge,type AudioTrimEdge} from "./audioTrim";

type Props={track:AudioTrack;trackNumber:number;sourceDuration:number;projectDuration:number;disabled:boolean;onCommit:(next:AudioTrack)=>void};
type Gesture={pointerId:number;x:number;pixelsPerSecond:number;edge:AudioTrimEdge;track:AudioTrack;node:HTMLButtonElement};
const format=(seconds:number)=>seconds.toFixed(2)+" seconds";

/** Draft audio edge changes stay local until release. Only a completed gesture makes one project-history entry. */
export function AudioTrimHandles({track,trackNumber,sourceDuration,projectDuration,disabled,onCommit}:Props){
  const [draft,setDraft]=useState<AudioTrack|null>(null);
  const gesture=useRef<Gesture|null>(null);
  const latest=useRef({track,sourceDuration,projectDuration,disabled});
  latest.current={track,sourceDuration,projectDuration,disabled};
  function cancel(){
    const current=gesture.current;gesture.current=null;setDraft(null);
    if(current?.node.hasPointerCapture?.(current.pointerId))current.node.releasePointerCapture(current.pointerId);
  }
  useEffect(()=>{cancel();},[track,sourceDuration,projectDuration,disabled]);
  useEffect(()=>{
    const onBlur=()=>cancel();
    window.addEventListener("blur",onBlur);
    return()=>{window.removeEventListener("blur",onBlur);gesture.current=null;};
  },[]);
  function isValid(g:Gesture){return !latest.current.disabled&&latest.current.track===g.track;}
  function candidate(g:Gesture,x:number){
    return trimAudioEdge(g.track,g.edge,(x-g.x)/g.pixelsPerSecond,sourceDuration,projectDuration);
  }
  function begin(event:PointerEvent<HTMLButtonElement>,edge:AudioTrimEdge){
    if(disabled||event.button!==0)return;
    const lane=event.currentTarget.closest(".sequence-audio-lane");
    const laneWidth=lane?.getBoundingClientRect().width??0;
    const scale=laneWidth/projectDuration;
    if(!Number.isFinite(scale)||scale<=0)return;
    event.preventDefault();event.stopPropagation();
    event.currentTarget.focus();
    gesture.current={pointerId:event.pointerId,x:event.clientX,pixelsPerSecond:scale,edge,track,node:event.currentTarget};
    event.currentTarget.setPointerCapture?.(event.pointerId);
    setDraft(track);
  }
  const shown=draft??track;
  return <>
    {draft&&<output className="sequence-audio-trim-feedback" role="status">{format(shown.start_seconds)} – {format(shown.start_seconds+shown.duration_seconds)}</output>}
    {(["start","end"] as const).map(edge=><button
      key={edge} type="button" role="slider" disabled={disabled}
      className={`sequence-audio-trim-handle audio-trim-${edge}`}
      aria-label={`Trim ${edge} of audio track ${trackNumber}`}
      aria-orientation="horizontal"
      aria-valuemin={edge==="start"?Math.max(0,track.start_seconds-track.source_in_seconds):track.start_seconds+0.04}
      aria-valuemax={edge==="start"?track.start_seconds+track.duration_seconds-0.04:Math.min(projectDuration,track.start_seconds+sourceDuration-track.source_in_seconds)}
      aria-valuenow={edge==="start"?shown.start_seconds:shown.start_seconds+shown.duration_seconds}
      aria-valuetext={format(edge==="start"?shown.start_seconds:shown.start_seconds+shown.duration_seconds)}
      title="Drag to trim. Arrow keys nudge 0.01s; Shift nudges 0.1s; Escape cancels."
      onPointerDown={event=>begin(event,edge)}
      onPointerMove={event=>{const g=gesture.current;if(g&&g.pointerId===event.pointerId&&isValid(g))setDraft(candidate(g,event.clientX)??g.track);}}
      onPointerUp={event=>{const g=gesture.current;if(!g||g.pointerId!==event.pointerId)return;const next=candidate(g,event.clientX);const valid=isValid(g);cancel();if(valid&&next)onCommit(next);}}
      onPointerCancel={cancel}
      onLostPointerCapture={()=>{if(gesture.current)cancel();}}
      onKeyDown={event=>{
        if(event.key==="Escape"){event.preventDefault();cancel();return;}
        if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;
        event.preventDefault();
        if(disabled||gesture.current)return;
        const multiplier=event.shiftKey?10:1;
        const delta=event.key==="Home"?-1e6:event.key==="End"?1e6:(event.key==="ArrowLeft"?-0.01:0.01)*multiplier;
        const next=trimAudioEdge(track,edge,delta,sourceDuration,projectDuration);
        if(next)onCommit(next);
      }}
    />)}
  </>;
}
