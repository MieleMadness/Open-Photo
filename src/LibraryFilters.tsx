import {useEffect,useRef,type ReactNode} from "react";
/** Filter controls remain ordinary labeled selects inside a dismissible disclosure. */
export function LibraryFilters({children}:{children:ReactNode}){
  const host=useRef<HTMLDetailsElement>(null);
  useEffect(()=>{const close=(e:PointerEvent)=>{if(host.current&&!host.current.contains(e.target as Node))host.current.open=false;};document.addEventListener("pointerdown",close);return()=>document.removeEventListener("pointerdown",close);},[]);
  return <details ref={host} className="library-filters" onBlur={e=>{if(e.relatedTarget&&!e.currentTarget.contains(e.relatedTarget as Node))e.currentTarget.open=false;}} onKeyDown={e=>{if(e.key==="Escape"){e.preventDefault();e.currentTarget.open=false;e.currentTarget.querySelector("summary")?.focus();}}}><summary className="secondary-button">Filters & sort</summary><div>{children}</div></details>;
}
