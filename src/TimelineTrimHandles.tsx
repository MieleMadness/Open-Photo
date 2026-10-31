import {useEffect,useRef,useState,type PointerEvent} from "react";
import {clockTime} from "./videoTime";

export type TrimRange={start:number;end:number};
export type TrimEdge="start"|"end";
/** Nudge relative to the original mark, preserving existing fractional source timing. */
export function trimRange(range:TrimRange,edge:TrimEdge,delta:number,step:number,limit:number,minimum:number):TrimRange{
  const snapped=Math.round(delta/step)*step,keep=Math.min(minimum,range.end-range.start);
  if(snapped===0)return {...range};
  return edge==="start"
    ?{...range,start:Math.max(0,Math.min(range.end-keep,range.start+snapped))}
    :{...range,end:Math.min(limit,Math.max(range.start+keep,range.end+snapped))};
}

type Props={range:TrimRange;limit:number;minimum:number;step:number;selected:boolean;disabled:boolean;token:object;index:number;pixelsPerSecond:()=>number;onCommit:(range:TrimRange)=>void;onPreview?:(seconds:number)=>void};
type Gesture={id:number;x:number;scale:number;range:TrimRange;edge:TrimEdge;token:object;node:HTMLButtonElement};

/** Pointer drafts never enter autosave. A successful release commits one shared trim command. */
export function TimelineTrimHandles({range,limit,minimum,step,selected,disabled,token,index,pixelsPerSecond,onCommit,onPreview}:Props){
  const keep=Math.min(minimum,range.end-range.start);
  const gesture=useRef<Gesture|null>(null),latest=useRef({token,disabled,selected});latest.current={token,disabled,selected};
  const [draft,setDraft]=useState<TrimRange|null>(null);
  function cancel(){
    const current=gesture.current;gesture.current=null;setDraft(null);
    if(current?.node.hasPointerCapture?.(current.id))current.node.releasePointerCapture(current.id);
  }
  useEffect(()=>{cancel();},[token,disabled,selected]);
  useEffect(()=>{
    const blur=()=>cancel();window.addEventListener("blur",blur);
    return()=>{window.removeEventListener("blur",blur);gesture.current=null;};
  },[]);
  function candidate(current:Gesture,x:number){return trimRange(current.range,current.edge,(x-current.x)/current.scale,step,limit,minimum);}
  function preview(current:Gesture,x:number){const next=candidate(current,x);setDraft(next);onPreview?.(next[current.edge]);}
  function valid(current:Gesture){return latest.current.token===current.token&&!latest.current.disabled&&latest.current.selected;}
  function begin(event:PointerEvent<HTMLButtonElement>,edge:TrimEdge){
    if(disabled||event.button!==0)return;
    const scale=pixelsPerSecond();if(!Number.isFinite(scale)||scale<=0)return;
    event.preventDefault();event.stopPropagation();event.currentTarget.focus({preventScroll:true});
    gesture.current={id:event.pointerId,x:event.clientX,scale,range:{...range},edge,token,node:event.currentTarget};
    event.currentTarget.setPointerCapture?.(event.pointerId);setDraft(range);onPreview?.(range[edge]);
  }
  if(!selected)return null;
  return <>
    {draft&&<><span className="timeline-trim-ghost" aria-hidden="true" style={{left:`${(draft.start-range.start)/(range.end-range.start)*100}%`,width:`${(draft.end-draft.start)/(range.end-range.start)*100}%`}}/><output className="timeline-trim-feedback" role="status">{clockTime(draft.start)} → {clockTime(draft.end)}</output></>}
    {(["start","end"] as const).map(edge=><button key={edge} type="button" role="slider" className={`timeline-trim-handle trim-${edge}`} disabled={disabled}
      aria-label={`Trim ${edge} of clip ${index+1}`} aria-orientation="horizontal" aria-valuemin={edge==="start"?0:range.start+keep} aria-valuemax={edge==="start"?range.end-keep:limit} aria-valuenow={(draft||range)[edge]} aria-valuetext={clockTime((draft||range)[edge])}
      title={`Drag to trim ${edge}. Arrow keys nudge; Shift moves ten steps. Escape cancels.`}
      onPointerDown={event=>begin(event,edge)} onPointerMove={event=>{const current=gesture.current;if(current?.id===event.pointerId&&valid(current))preview(current,event.clientX);}}
      onPointerUp={event=>{const current=gesture.current;if(!current||current.id!==event.pointerId)return;const next=candidate(current,event.clientX),canCommit=valid(current);if(canCommit)onPreview?.(next[current.edge]);cancel();if(canCommit&&(next.start!==current.range.start||next.end!==current.range.end))onCommit(next);}}
      onPointerCancel={cancel} onLostPointerCapture={()=>{if(gesture.current)cancel();}}
      onKeyDown={event=>{
        if(event.key==="Escape"){event.preventDefault();cancel();return;}
        if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;
        event.preventDefault();if(disabled||gesture.current)return;
        const delta=event.key==="Home"?-limit:event.key==="End"?limit:(event.key==="ArrowLeft"?-1:1)*step*(event.shiftKey?10:1);
        const next=trimRange(range,edge,delta,step,limit,minimum);
        if(next.start!==range.start||next.end!==range.end){onPreview?.(next[edge]);onCommit(next);}
      }}/>) }
  </>;
}
