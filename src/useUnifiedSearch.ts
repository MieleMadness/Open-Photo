import {useEffect,useMemo,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
import {mediaIdentity,parseSearch,rankSearch,projectLiveEvidence,type SearchEvidence,type SearchMedia} from "./unifiedSearch";
type NativeResponse={evidence:SearchEvidence[];warnings:string[];cancelled:boolean};
/** Keep one completed view while a newer query is debounced or awaiting evidence. */
export function useUnifiedSearch({photos,root,workspace,query,tags,ratings,aiEnabled,desktop,device,indexing,refresh,livePairs}:{photos:SearchMedia[]|undefined;root:string;workspace:string;query:string;tags:Record<string,string[]>;ratings:Record<string,number>;aiEnabled:boolean;desktop:boolean;device:string;indexing:boolean;refresh:number;livePairs?:Map<string,SearchMedia>}){
  const parsed=useMemo(()=>parseSearch(query),[query]),context=JSON.stringify([workspace,root,aiEnabled,device]);
  const [submitted,setSubmitted]=useState<{query:string;context:string;nonce:number}|null>(null);
  const [pending,setPending]=useState(false);
  const [completed,setCompleted]=useState<{context:string;photos:SearchMedia[]|undefined;query:string;evidence:SearchEvidence[];warnings:string[];refresh:number;indexing:boolean}|null>(null);
  useEffect(()=>{
    if(!photos||parsed.error||!query.trim()){setPending(false);return;}
    setPending(true);
    let active=true;const requestId=crypto.randomUUID();
    const finish=(evidence:SearchEvidence[],warnings:string[])=>{if(active){setCompleted({context,photos,query,evidence:projectLiveEvidence(photos,root,livePairs,evidence),warnings,refresh,indexing});setPending(false);}};
    const timer=setTimeout(()=>{void(async()=>{
      if(!desktop||!workspace||!aiEnabled||indexing||(!parsed.text&&!parsed.person)){finish([],indexing?["Content indexes are being updated."]:[]);return;}
      try{
        await flushSettings(["openphoto.workspaces"]);if(!active)return;
        const result=await invoke<NativeResponse>("search_workspace_evidence",{request:{requestId,workspaceId:workspace,query:parsed.text,terms:parsed.terms,personQuery:parsed.person,includeAi:true,includeSemantic:!!parsed.text,device}});
        if(!result||!Array.isArray(result.evidence)||!Array.isArray(result.warnings))throw Error("The search response was incomplete.");
        if(!result.cancelled)finish(result.evidence,result.warnings);
      }catch(error){finish([],[`Indexed content could not be searched: ${String(error)}`]);}
    })();},submitted?.query===query&&submitted.context===context?0:400);
    return()=>{active=false;clearTimeout(timer);if(desktop)void invoke("cancel_workspace_search",{requestId}).catch(()=>{});};
  },[query,context,photos,desktop,workspace,aiEnabled,indexing,refresh,parsed,submitted,device,livePairs,root]);
  const current=completed?.context===context&&completed.photos===photos?completed:null;
  const displayQuery=query.trim()?current?.query||"":"",displayParsed=useMemo(()=>parseSearch(displayQuery),[displayQuery]);
  const results=useMemo(()=>rankSearch(photos||[],root,displayParsed,tags,ratings,current?.evidence||[]),[photos,root,displayParsed,tags,ratings,current]);
  const byId=useMemo(()=>new Map(results.map(result=>[result.media_id,result])),[results]);
  const busy=!!query.trim()&&!parsed.error&&!!photos&&(pending||!current||current.query!==query||current.refresh!==refresh||current.indexing!==indexing);
  return {parsed,results,active:!!query.trim(),busy,warnings:current?.warnings||[],match:(photo:SearchMedia)=>byId.get(mediaIdentity(photo.libraryRoot||root,photo.path)),submit:()=>setSubmitted(previous=>({query,context,nonce:(previous?.nonce||0)+1}))};
}
