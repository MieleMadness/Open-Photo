import {useLayoutEffect,useRef,useState,type RefObject} from "react";
import {saveSetting} from "./userData";

const key="openphoto.editor.timelineRatio",defaultRatio=.32;
function savedRatio(){const value=localStorage.getItem(key),number=Number(value);return value!==null&&Number.isFinite(number)&&number>0&&number<1?number:defaultRatio;}
export function timelineSplitBounds(height:number,occupied:number){
  const available=Math.floor(Math.max(0,height-occupied-10)),minimum=Math.min(260,Math.max(120,available-160),available);
  return {minimum,maximum:Math.max(minimum,available-160),available};
}
const clamp=(value:number,minimum:number,maximum:number)=>Math.round(Math.max(minimum,Math.min(maximum,value)));

/** Layout preference only: resizing never edits a project or creates an undo entry. */
export function EditorTimelineDivider({host,onHeight}:{host:RefObject<HTMLElement|null>;onHeight:(height:number)=>void}){
  const preference=useRef(savedRatio()),geometry=useRef({height:800,occupied:80});
  const current=useRef(260),gesture=useRef<{y:number;height:number;id:number;node:HTMLDivElement}|null>(null);
  const [height,setHeight]=useState(260),[bounds,setBounds]=useState(()=>timelineSplitBounds(800,80));
  function show(value:number){const limits=timelineSplitBounds(geometry.current.height,geometry.current.occupied),next=clamp(value,limits.minimum,limits.maximum);current.current=next;setHeight(next);onHeight(next);}
  function cancel(){const active=gesture.current;gesture.current=null;show(geometry.current.height*preference.current);if(active?.node.hasPointerCapture?.(active.id))active.node.releasePointerCapture(active.id);}
  function save(){preference.current=current.current/geometry.current.height;saveSetting(key,String(preference.current));}
  useLayoutEffect(()=>{
    const root=host.current;if(!root)return;
    const header=root.querySelector<HTMLElement>(".editor-project-bar"),feedback=root.querySelector<HTMLElement>(".editor-feedback");
    const measure=()=>{geometry.current={height:root.clientHeight||800,occupied:(header?.getBoundingClientRect().height||80)+(feedback?.getBoundingClientRect().height||0)};setBounds(timelineSplitBounds(geometry.current.height,geometry.current.occupied));if(!gesture.current)show(geometry.current.height*preference.current);};
    measure();const observer=typeof ResizeObserver!=="undefined"?new ResizeObserver(measure):null;
    observer?.observe(root);if(header)observer?.observe(header);if(feedback)observer?.observe(feedback);
    const blur=()=>{if(gesture.current)cancel();};window.addEventListener("blur",blur);
    return()=>{observer?.disconnect();window.removeEventListener("blur",blur);gesture.current=null;};
  },[host,onHeight]);
  return <div className="editor-timeline-divider" role="separator" tabIndex={0} aria-label="Resize preview and timeline" aria-orientation="horizontal" aria-valuemin={bounds.minimum} aria-valuemax={bounds.maximum} aria-valuenow={height} aria-valuetext={`Timeline ${height} pixels. Preview ${Math.max(0,bounds.available-height)} pixels.`}
    title="Drag to resize. Arrow keys adjust; Home/End use limits. Double-click to reset. Escape cancels."
    onPointerDown={event=>{if(event.button!==0)return;event.preventDefault();event.currentTarget.focus({preventScroll:true});gesture.current={y:event.clientY,height:current.current,id:event.pointerId,node:event.currentTarget};event.currentTarget.setPointerCapture?.(event.pointerId);}}
    onPointerMove={event=>{const active=gesture.current;if(active?.id===event.pointerId)show(active.height+active.y-event.clientY);}}
    onPointerUp={event=>{const active=gesture.current;if(active?.id!==event.pointerId)return;show(active.height+active.y-event.clientY);gesture.current=null;if(active.node.hasPointerCapture?.(active.id))active.node.releasePointerCapture(active.id);if(current.current!==active.height)save();}}
    onPointerCancel={cancel} onLostPointerCapture={()=>{if(gesture.current)cancel();}}
    onDoubleClick={()=>{gesture.current=null;preference.current=defaultRatio;show(geometry.current.height*defaultRatio);saveSetting(key,String(defaultRatio));}}
    onKeyDown={event=>{if(event.key==="Escape"){if(gesture.current){event.preventDefault();cancel();}return;}if(!["ArrowUp","ArrowDown","Home","End"].includes(event.key))return;event.preventDefault();if(gesture.current)return;const previous=current.current;show(event.key==="Home"?bounds.minimum:event.key==="End"?bounds.maximum:current.current+(event.key==="ArrowUp"?1:-1)*(event.shiftKey?60:20));if(current.current!==previous)save();}}/>
}
