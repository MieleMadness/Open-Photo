import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {Film} from "lucide-react";

type Job={cancelled:boolean;run:()=>Promise<void>};
const waiting:Job[]=[],cache=new Map<string,string>();
let running=0;
function pump(){
  while(running<2&&waiting.length){
    const job=waiting.shift()!;if(job.cancelled)continue;
    running++;void job.run().finally(()=>{running--;pump();});
  }
}

/** Lazy, bounded posters never start a decoder for every item in the media bin at once. */
export function EditorPoster({root,path,identity,filmstrip=false}:{root?:string;path?:string;identity:string;filmstrip?:boolean}){
  const host=useRef<HTMLSpanElement>(null),[source,setSource]=useState("");
  useEffect(()=>{
    setSource("");if(!root||!path)return;
    const key=JSON.stringify([root,path,identity]),job:Job={cancelled:false,run:async()=>{
      try{
        const image=cache.get(key)||await invoke<string>("video_poster",{root,path,maxEdge:320});
        if(typeof image!=="string")return;
        cache.set(key,image);while(cache.size>100)cache.delete(cache.keys().next().value!);
        if(!job.cancelled)setSource(image);
      }catch{/* A missing poster must not prevent selecting or relinking its source. */}
    }};
    let started=false;
    const start=()=>{if(started||job.cancelled)return;started=true;observer?.disconnect();waiting.push(job);pump();};
    const observer=typeof IntersectionObserver!=="undefined"?new IntersectionObserver(entries=>{if(entries.some(e=>e.isIntersecting))start();},{rootMargin:"120px"}):null;
    if(observer&&host.current)observer.observe(host.current);else start();
    return()=>{job.cancelled=true;observer?.disconnect();};
  },[root,path,identity]);
  return <span ref={host} className={filmstrip?"editor-poster editor-filmstrip":"editor-poster"} aria-hidden="true">{source?(filmstrip?<span className="editor-filmstrip-frames">{[0,1,2,3].map(frame=><img key={frame} src={source} alt="" loading="lazy"/>)}</span>:<img src={source} alt="" loading="lazy"/>):<Film size={24}/>}</span>;
}
