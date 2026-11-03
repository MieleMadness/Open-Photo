import {LayoutGrid,List} from "lucide-react";
import {useState} from "react";
import {saveSetting} from "./userData";
export type BrowseMode="list"|"tiles";
/** Workspace-scoped views are independent of the media grid and never schedule I/O. */
export function useBrowseView(workspace:string,section:"folders"|"collections"):[BrowseMode,(mode:BrowseMode)=>void]{
  const key=`openphoto.browse.${section}.${workspace}`;
  const [choices,setChoices]=useState<Record<string,BrowseMode>>({});
  const mode=choices[key]||(localStorage.getItem(key)==="tiles"?"tiles":"list");
  return [mode,value=>{setChoices(previous=>({...previous,[key]:value}));saveSetting(key,value);}];
}
export function BrowseView({label,value,onChange}:{label:string;value:BrowseMode;onChange:(mode:BrowseMode)=>void}){
  return <div className="view-switch browse-view" role="group" aria-label={label}>{(["list","tiles"] as const).map(mode=><button key={mode} aria-pressed={value===mode} className={value===mode?"active":""} onClick={()=>onChange(mode)}>{mode==="list"?<List size={16}/>:<LayoutGrid size={16}/>} {mode==="list"?"List":"Tiles"}</button>)}</div>;
}
