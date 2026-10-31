import {useEffect,useRef,type ReactNode} from "react";

/** Native disclosure with predictable outside-click and Escape dismissal. */
export function EditorProjectMenu({children}:{children:ReactNode}){
  const host=useRef<HTMLDetailsElement>(null);
  useEffect(()=>{
    const outside=(event:PointerEvent)=>{if(host.current?.open&&!host.current.contains(event.target as Node))host.current.open=false;};
    document.addEventListener("pointerdown",outside);
    return()=>document.removeEventListener("pointerdown",outside);
  },[]);
  return <details ref={host} className="editor-project-menu" onKeyDown={event=>{
    if(event.key==="Escape"&&host.current?.open){event.preventDefault();event.stopPropagation();host.current.open=false;host.current.querySelector("summary")?.focus();}
  }} onBlur={event=>{if(event.relatedTarget&&!event.currentTarget.contains(event.relatedTarget as Node))event.currentTarget.open=false;}}>
    <summary>Project</summary><div className="editor-project-menu-panel">{children}</div>
  </details>;
}
