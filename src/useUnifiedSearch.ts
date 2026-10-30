import {useEffect,useMemo,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
import {mediaIdentity,parseSearch,rankSearch,type SearchEvidence,type SearchMedia} from "./unifiedSearch";
type NativeResponse={evidence:SearchEvidence[];warnings:string[];cancelled:boolean};
const noEvidence:SearchEvidence[]=[];
export function useUnifiedSearch({photos,root,workspace,query,tags,ratings,aiEnabled,desktop,device,indexing,refresh}:{photos:SearchMedia[]|undefined;root:string;workspace:string;query:string;tags:Record<string,string[]>;ratings:Record<string,number>;aiEnabled:boolean;desktop:boolean;device:string;indexing:boolean;refresh:number}){
  const parsed=useMemo(()=>parseSearch(query),[query]),scope=JSON.stringify([workspace,root,query,aiEnabled,device]);
  const [submitted,setSubmitted]=useState<{scope:string;nonce:number}|null>(null);
  const [batch,setBatch]=useState<{scope:string;photos:SearchMedia[]|undefined;evidence:SearchEvidence[];warnings:string[];busy:boolean}>({scope:"",photos:undefined,evidence:[],warnings:[],busy:false});
  useEffect(()=>setSubmitted(null),[scope]);
  useEffect(()=>{
    let active=true;const requestId=crypto.randomUUID();
    if(!desktop||!workspace||!photos||!aiEnabled||indexing||parsed.error||(!parsed.text&&!parsed.person)){setBatch({scope,photos,evidence:[],warnings:indexing?["Content indexes are being updated. Content matches will refresh when indexing finishes."]:[],busy:false});return;}
    setBatch(previous=>previous.scope===scope&&previous.photos===photos?{...previous,warnings:[],busy:true}:{scope,photos,evidence:[],warnings:[],busy:true});
    const timer=setTimeout(()=>{void (async()=>{
      try{
        await flushSettings(["openphoto.workspaces"]);if(!active)return;
        const result=await invoke<NativeResponse>("search_workspace_evidence",{request:{requestId,workspaceId:workspace,query:parsed.text,terms:parsed.terms,personQuery:parsed.person,includeAi:true,includeSemantic:submitted?.scope===scope,device}});
        if(active){if(!result||!Array.isArray(result.evidence)||!Array.isArray(result.warnings))throw Error("The search response was incomplete.");setBatch({scope,photos,evidence:result.evidence,warnings:result.warnings,busy:false});}
      }catch(error){if(active)setBatch({scope,photos,evidence:[],warnings:[`Indexed content could not be searched: ${String(error)}`],busy:false});}
    })();},submitted?.scope===scope?0:300);
    return()=>{active=false;clearTimeout(timer);void invoke("cancel_workspace_search",{requestId}).catch(()=>{});};
  },[scope,photos,desktop,workspace,aiEnabled,indexing,refresh,parsed,submitted,device]);
  const current=batch.scope===scope&&batch.photos===photos;
  const evidence=current?batch.evidence:noEvidence;
  const results=useMemo(()=>query.trim()?rankSearch(photos||[],root,parsed,tags,ratings,evidence):[],[photos,root,parsed,tags,ratings,evidence,query]);
  const byId=useMemo(()=>new Map(results.map(result=>[result.media_id,result])),[results]);
  return {parsed,results,active:!!query.trim(),busy:current&&batch.busy,warnings:current?batch.warnings:[],submitted:submitted?.scope===scope,match:(photo:SearchMedia)=>byId.get(mediaIdentity(photo.libraryRoot||root,photo.path)),submit:()=>setSubmitted(previous=>({scope,nonce:(previous?.nonce||0)+1}))};
}
