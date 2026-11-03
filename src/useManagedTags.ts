import {useEffect,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {readMetadataBatches,type RootedPath} from "./metadataBatches";

export type ManagedTagEntry={path:string;tags:string[]};

export function useManagedTags(photos:RootedPath[]|undefined,root:string,workspace:string,enabled:boolean,refresh:number){
  const [managedTags,setManagedTags]=useState<Record<string,string[]>>({});
  const [tagIndexBusy,setTagIndexBusy]=useState(false);
  const [tagIndexError,setTagIndexError]=useState("");
  useEffect(()=>{setManagedTags({});setTagIndexError("");setTagIndexBusy(false)},[workspace,root]);
  useEffect(()=>{
    if(!enabled||!photos){setTagIndexBusy(false);return;}
    let active=true;
    setTagIndexBusy(true);setTagIndexError("");
    void readMetadataBatches(photos,root,1000,(parent,paths)=>invoke<ManagedTagEntry[]>("read_managed_tags",{root:parent,paths}),()=>active)
      .then(({values,errors})=>{
        if(!active)return;
        setManagedTags(Object.fromEntries(values.map(entry=>[entry.path,entry.tags])));
        setTagIndexError(errors.length?`Some tags could not be loaded: ${errors.join("; ")}`:"");
      }).finally(()=>{if(active)setTagIndexBusy(false)});
    return()=>{active=false};
  },[photos,root,workspace,enabled,refresh]);
  return {managedTags,tagIndexBusy,tagIndexError};
}
