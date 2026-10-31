import {useId,type ReactNode} from "react";
import {X} from "lucide-react";
export type InspectorTab="clip"|"canvas";

/** Hidden panels remain mounted so switching tasks preserves unfinished numeric drafts. */
export function EditorInspector({open,tab,onTab,onClose,clip,canvas}:{open:boolean;tab:InspectorTab;onTab:(tab:InspectorTab)=>void;onClose:()=>void;clip:ReactNode;canvas:ReactNode}){
  const id=useId(),tabs=[{id:"clip",label:"Clip"},{id:"canvas",label:"Canvas & export"}] as const;
  return <aside className="editor-inspector" aria-label="Clip inspector" hidden={!open}>
    <div className="editor-panel-heading"><h2>Controls</h2><button className="icon-button" aria-label="Hide clip controls" title="Hide clip controls" onClick={onClose}><X size={18}/></button></div>
    <div className="editor-inspector-tabs" role="tablist" aria-label="Editor controls">{tabs.map((item,index)=><button key={item.id} className="editor-inspector-tab" role="tab" data-inspector-tab={item.id} id={`${id}-${item.id}-tab`} aria-controls={`${id}-${item.id}`} aria-selected={tab===item.id} tabIndex={tab===item.id?0:-1} onClick={()=>onTab(item.id)} onKeyDown={event=>{
      if(!["ArrowLeft","ArrowRight","Home","End"].includes(event.key))return;
      event.preventDefault();const next=event.key==="Home"?0:event.key==="End"?1:(index+(event.key==="ArrowRight"?1:-1)+2)%2;
      onTab(tabs[next].id);event.currentTarget.parentElement?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[next]?.focus();
    }}>{item.label}</button>)}</div>
    <div role="tabpanel" id={`${id}-clip`} aria-labelledby={`${id}-clip-tab`} hidden={tab!=="clip"}>{clip}</div>
    <div role="tabpanel" id={`${id}-canvas`} aria-labelledby={`${id}-canvas-tab`} hidden={tab!=="canvas"}>{canvas}</div>
  </aside>;
}
