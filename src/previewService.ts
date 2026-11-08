import {invoke} from "@tauri-apps/api/core";
import {ThumbnailCache} from "./thumbnailCache";

export type PreviewRequest={root:string;path:string;maxEdge:number;sourceVersion?:string|null;bytes:number;modified:number;kind:"image"|"video"|"heic"};
type Job={key:string;request:PreviewRequest;controller:AbortController;listeners:Set<(value?:string)=>void>;started:boolean};
type Load=(request:PreviewRequest,signal:AbortSignal)=>Promise<string>;
/** One owner for deduplication, concurrency, cancellation and content-versioned caches. */
export class PreviewService {
  private waiting:Job[]=[];
  private jobs=new Map<string,Job>();
  private cache=new ThumbnailCache();
  private running=0;
  private heicRunning=0;
  constructor(private load:Load=loadPreview){}
  subscribe(request:PreviewRequest,notify:(value?:string)=>void){
    const key=JSON.stringify([request.root,request.path,request.sourceVersion??null,request.bytes,request.modified,request.maxEdge,request.kind]);
    const cached=request.sourceVersion?this.cache.get(key):undefined;
    if(cached){notify(cached);return()=>{};}
    let job=this.jobs.get(key);
    if(!job){job={key,request,controller:new AbortController(),listeners:new Set(),started:false};this.jobs.set(key,job);this.waiting.push(job);}
    job.listeners.add(notify);this.pump();
    return()=>{
      job.listeners.delete(notify);
      if(!job.listeners.size){job.controller.abort();if(this.jobs.get(key)===job)this.jobs.delete(key);this.pump();}
    };
  }
  private pump(){
    this.waiting=this.waiting.filter(job=>!job.controller.signal.aborted);
    while(this.running<3){
      const index=this.waiting.findIndex(job=>job.request.kind!=="heic"||this.heicRunning<1);
      if(index<0)return;
      const job=this.waiting.splice(index,1)[0];job.started=true;this.running++;
      if(job.request.kind==="heic")this.heicRunning++;
      void this.execute(job);
    }
  }
  private async execute(job:Job){
    let value:string|undefined;
    try{
      const result=await this.load(job.request,job.controller.signal);
      if(!job.controller.signal.aborted&&typeof result==="string"&&result){value=result;if(job.request.sourceVersion)this.cache.set(job.key,result);}
    }catch{/* The caller retains its preview placeholder on a decode or identity failure. */}
    finally{
      if(this.jobs.get(job.key)===job)this.jobs.delete(job.key);
      this.running--;if(job.request.kind==="heic")this.heicRunning--;
      for(const notify of job.listeners)notify(value);
      this.pump();
    }
  }
}
async function loadPreview(request:PreviewRequest,signal:AbortSignal){
  const {root,path,maxEdge,sourceVersion,kind}=request;
  const binding=sourceVersion?{expectedSourceVersion:sourceVersion}:{};
  if(kind!=="heic")return invoke<string>(kind==="video"?"video_poster":"preview_image",{root,path,maxEdge,...binding});
  const encoded=await invoke<string>("read_iphone_media",{root,path,...binding});
  if(signal.aborted)throw new DOMException("Cancelled","AbortError");
  const {decodeHeicPreview}=await import("./heicPreview");
  if(signal.aborted)throw new DOMException("Cancelled","AbortError");
  return decodeHeicPreview(encoded,maxEdge,signal);
}
export const libraryPreviews=new PreviewService();
