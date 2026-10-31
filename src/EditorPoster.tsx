import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
import {Film,Image as ImageIcon} from "lucide-react";

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
export function EditorPoster({root,path,identity,filmstrip=false,photo=false,clipStart=0,clipEnd=0}:{root?:string;path?:string;identity:string;filmstrip?:boolean;photo?:boolean;clipStart?:number;clipEnd?:number}){
  const host=useRef<HTMLSpanElement>(null),[source,setSource]=useState("");
  useEffect(()=>{
    setSource("");if(!root||!path)return;
    const key=JSON.stringify([root,path,identity,photo]),job:Job={cancelled:false,run:async()=>{
      try{
        const image=cache.get(key)||await invoke<string>(photo?"preview_image":"video_poster",{root,path,maxEdge:320});
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
  },[root,path,identity,photo]);
  if(filmstrip&&!photo)return <TimelineFrames root={root} path={path} identity={identity} start={clipStart} end={clipEnd} fallback={source} host={host}/>;
  return <span ref={host} className={filmstrip?"editor-poster editor-filmstrip":"editor-poster"} aria-hidden="true">{source?(filmstrip?<span className="editor-filmstrip-frames">{[0,1,2,3].map(frame=><img key={frame} src={source} alt="" loading="lazy"/>)}</span>:<img src={source} alt="" loading="lazy"/>):photo?<ImageIcon size={24}/>:<Film size={24}/>}</span>;
}

/** One request lane for visible timeline clips; hidden/unmounted clips don't queue more work. */
const frameCache=new Map<string,string>();
type FrameRequest={key:string;cancelled:boolean;run:()=>Promise<void>};
const frameWaiting:FrameRequest[]=[];
let frameRunning=0;
function pumpFrames(){
  while(frameRunning<2&&frameWaiting.length){
    const job=frameWaiting.shift()!;
    if(job.cancelled)continue;
    frameRunning++;
    void job.run().finally(()=>{frameRunning--;pumpFrames();});
  }
}
function TimelineFrames({root,path,identity,start,end,fallback,host}:{root?:string;path?:string;identity:string;start:number;end:number;fallback:string;host:React.RefObject<HTMLSpanElement|null>}){
  const [frames,setFrames]=useState<string[]>([]);
  const [visible,setVisible]=useState(false);
  const [frameCount,setFrameCount]=useState(4);
  useEffect(()=>{
    const element=host.current;
    if(!element)return;
    const measure=()=>setFrameCount(Math.max(1,Math.min(8,Math.ceil((element.getBoundingClientRect().width||320)/90))));
    measure();
    if(typeof ResizeObserver==="undefined")return;
    const observer=new ResizeObserver(measure);
    observer.observe(element);
    return()=>observer.disconnect();
  },[host]);
  useEffect(()=>{
    const element=host.current;
    if(!element)return;
    if(typeof IntersectionObserver==="undefined"){setVisible(true);return;}
    const observer=new IntersectionObserver(entries=>setVisible(entries.some(e=>e.isIntersecting)),{rootMargin:"100px"});
    observer.observe(element);
    return()=>observer.disconnect();
  },[host]);
  useEffect(()=>{
    let cancelled=false;
    const jobs:FrameRequest[]=[];
    if(!visible||!root||!path||!Number.isFinite(start)||!Number.isFinite(end)||end<=start){setFrames([]);return;}
    const timestamps=Array.from({length:frameCount},(_,i)=>start+(end-start)*(i+.5)/frameCount);
    const keys=timestamps.map(time=>JSON.stringify([root,path,identity,Math.round(time*1000)]));
    const initial=keys.map(key=>frameCache.get(key)||"");
    setFrames(initial);
    // A brief debounce collapses rapid zoom/resize changes before queuing native work.
    const timer=window.setTimeout(()=>{
      if(cancelled)return;
      keys.forEach((key,index)=>{
        if(frameCache.has(key))return;
        const job:FrameRequest={key,cancelled:false,run:async()=>{
          if(cancelled||job.cancelled)return;
          try{
            const image=await invoke<string>("video_timeline_frame",{root,path,seconds:timestamps[index]});
            if(typeof image==="string"&&image){
              frameCache.set(key,image);
              while(frameCache.size>96)frameCache.delete(frameCache.keys().next().value!);
              if(!cancelled)setFrames(previous=>{const next=[...previous];next[index]=image;return next;});
            }
          }catch{/* Poster fallback is preserved if the decoder rejects a frame. */}
        }};
        jobs.push(job);frameWaiting.push(job);
      });
      pumpFrames();
    },120);
    return()=>{cancelled=true;window.clearTimeout(timer);jobs.forEach(job=>{job.cancelled=true;});};
  },[visible,root,path,identity,start,end,frameCount]);
  return <span ref={host} className="editor-poster editor-filmstrip" aria-hidden="true"><span className="editor-filmstrip-frames">{Array.from({length:frameCount},(_,index)=><img key={index} style={{flexBasis:`${100/frameCount}%`,width:`${100/frameCount}%`}} src={frames[index]||fallback||undefined} alt="" loading="lazy"/>)}</span></span>;
}
