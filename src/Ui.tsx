import {createContext,useContext,useId,useLayoutEffect,useRef,useState,type ReactNode} from "react";
import {createPortal} from "react-dom";

const focusable='button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href],[tabindex="0"],summary';

export const NoticeTarget=createContext<HTMLElement|null>(null);
export const NoticeHostSetter=createContext<(host:HTMLDivElement|null)=>void>(()=>{});
/** Keep job/save controls in the active modal without remounting their state owners. */
export function NoticeSurface({children}:{children:ReactNode}){const target=useContext(NoticeTarget);return target?createPortal(children,target):<>{children}</>;}

/** Native modality makes the underlying app inert; the explicit boundary also supports test hosts. */
export function Modal({label,className="",onClose,children}:{label:string;className?:string;onClose:()=>void;children:ReactNode}){
  const dialog=useRef<HTMLDialogElement>(null),close=useRef(onClose);
  const setNoticeHost=useContext(NoticeHostSetter);
  close.current=onClose;
  useLayoutEffect(()=>{const panel=dialog.current?.firstElementChild as HTMLElement|null;if(panel)panel.scrollTop=0;},[className]);
  useLayoutEffect(()=>{
    const element=dialog.current!,invoker=document.activeElement as HTMLElement|null;
    element.dataset.input=invoker?.matches(":focus-visible")?"keyboard":"pointer";
    if(typeof element.showModal==="function")element.showModal();else element.setAttribute("open","");
    const target=element.querySelector<HTMLElement>("[data-initial-focus]")||element.querySelector<HTMLElement>(focusable);
    target?.focus();
    return()=>{
      if(typeof element.close==="function"&&element.open)element.close();
      const fallback=document.querySelector<HTMLElement>('.nav-button[aria-current="page"]');
      (invoker?.isConnected?invoker:fallback)?.focus();
    };
  },[]);
  return createPortal(<dialog ref={dialog} className="modal-backdrop" aria-label={label} aria-modal="true"
    onCancel={event=>{event.preventDefault();close.current();}}
    onClick={event=>{if(event.target===event.currentTarget)close.current();}}
    onKeyDown={event=>{
      if(event.key==="Escape"){event.preventDefault();event.stopPropagation();close.current();}
      if(event.key!=="Tab")return;
      const items=[...event.currentTarget.querySelectorAll<HTMLElement>(focusable)].filter(el=>{
        if(el.closest("[hidden],details:not([open])>*:not(summary)"))return false;
        for(let parent:HTMLElement|null=el;parent&&parent!==dialog.current;parent=parent.parentElement){const style=getComputedStyle(parent);if(style.display==="none"||style.visibility==="hidden")return false;}
        return true;
      });
      const first=items[0],last=items.at(-1);
      if(event.shiftKey&&document.activeElement===first){event.preventDefault();last?.focus();}
      else if(!event.shiftKey&&document.activeElement===last){event.preventDefault();first?.focus();}
    }}><section className={className}><div className="details-notices" ref={setNoticeHost}/>{children}</section></dialog>,document.body);
}

export function ErrorNotice({title,error,children}:{title:string;error:unknown;children?:ReactNode}){
  return <section className="feedback feedback-error" role="alert"><strong>{title}</strong><p>Try again. Your original media has not been changed.</p><details><summary>Show details</summary><p className="break-anywhere">{String(error)}</p></details>{children&&<div className="feedback-actions">{children}</div>}</section>;
}

type Field={name:string;label:string;value?:string;type?:"text"|"number";min?:number;max?:number;step?:number|"any";required?:boolean;options?:{value:string;label:string}[]};
export function TaskForm({title,fields,onClose,onSubmit,submitLabel="Save"}:{title:string;fields:Field[];onClose:()=>void;onSubmit:(values:Record<string,string>)=>void|Promise<void>;submitLabel?:string}){
  const [values,setValues]=useState(()=>Object.fromEntries(fields.map(f=>[f.name,f.value||""])));
  const [error,setError]=useState(""),[busy,setBusy]=useState(false);const working=useRef(false),id=useId();
  return <Modal label={title} className="task-dialog" onClose={()=>{if(!working.current)onClose();}}><h2>{title}</h2><form aria-busy={busy} onSubmit={async event=>{
    event.preventDefault();
    if(working.current)return;
    const clean=Object.fromEntries(Object.entries(values).map(([key,value])=>[key,value.trim()]));
    const missing=fields.find(field=>field.required!==false&&!clean[field.name]);
    if(missing){setError(`Enter ${missing.label.toLowerCase()} before saving.`);document.getElementById(`${id}-${missing.name}`)?.focus();return;}
    working.current=true;setBusy(true);setError("");
    try{await onSubmit(clean);onClose();}catch(cause){setError(String(cause));}finally{working.current=false;setBusy(false);}
  }}><fieldset className="form-fields" disabled={busy}>{fields.map((field,index)=><label key={field.name} htmlFor={`${id}-${field.name}`}>{field.label}{field.options?
    <select id={`${id}-${field.name}`} value={values[field.name]} onChange={event=>setValues({...values,[field.name]:event.target.value})}>{field.options.map(option=><option key={option.value} value={option.value}>{option.label}</option>)}</select>:
    <input id={`${id}-${field.name}`} data-initial-focus={index===0?"":undefined} type={field.type||"text"} required={field.required!==false} min={field.min} max={field.max} step={field.type==="number"?(field.step??1):undefined} maxLength={field.type!=="number"?200:undefined} aria-invalid={error&&!values[field.name].trim()?true:undefined} aria-describedby={error?`${id}-error`:undefined} value={values[field.name]} onChange={event=>{setValues({...values,[field.name]:event.target.value});setError("");}}/>}</label>)}</fieldset>
    {error&&<p id={`${id}-error`} role="alert" className="field-error">{error}</p>}<div className="feedback-actions"><button className="secondary-button" type="button" disabled={busy} onClick={onClose}>Cancel</button><button className="primary-button" type="submit" disabled={busy}>{busy?"Saving…":submitLabel}</button></div>
  </form></Modal>;
}
