import {useId,useState} from "react";

// Preserve invalid drafts so editing never silently uses an older value.
export function NumberField({label,value,min,max,step=1,onCommit}:{label:string;value:number;min:number;max:number;step?:number|"any";onCommit:(value:number)=>string|void}){
  const [previous,setPrevious]=useState(value),[draft,setDraft]=useState(String(value)),[error,setError]=useState("");const id=useId();
  // Synchronize only actual external edits (including undo), not a delayed mount effect.
  if(value!==previous){setPrevious(value);setDraft(String(value));setError("");}
  function commit(){const number=Number(draft);if(!draft.trim()||!Number.isFinite(number)||number<min||number>max||(typeof step==="number"&&Math.abs((number-min)/step-Math.round((number-min)/step))>0.00001)){setError(`Enter a value from ${min} to ${max}${step==="any"?"":`, in steps of ${step}`}.`);return;}setError(onCommit(number)||"");}
  return <label htmlFor={id}>{label}<input id={id} aria-label={label} aria-invalid={!!error} aria-describedby={error?`${id}-error`:undefined} type="number" required value={draft} min={min} max={max} step={step} onChange={event=>{setDraft(event.target.value);setError("");}} onBlur={commit} onKeyDown={event=>{if(event.key==="Enter")event.currentTarget.blur();}}/>{error&&<span id={`${id}-error`} className="field-error" role="alert">{error}</span>}</label>;
}
