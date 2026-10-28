import {invoke} from "@tauri-apps/api/core";
import {markSeconds,type VideoTime} from "./videoTime";
import type {VideoInfo} from "./VideoDetails";
export type Crop={x:number;y:number;width:number;height:number};
export type Edits={crop:Crop;quarter_turns:number;flip_horizontal:boolean;flip_vertical:boolean;muted:boolean;gain_percent:number};
export type VideoProject={schema_version:1;id:string;title:string;asset:{id:string;relative_path:string;sha256:string;bytes:number;info:VideoInfo};clips:{id:string;asset_id:string;source_in:VideoTime;source_out:VideoTime;edits:Edits}[];output:{width:number;height:number;fit:"fit"|"fill"}};
export type LoadedProject={project:VideoProject;revision:number;source_path:string|null;source_status:string};
export const defaultEdits=():Edits=>({crop:{x:0,y:0,width:10000,height:10000},quarter_turns:0,flip_horizontal:false,flip_vertical:false,muted:false,gain_percent:100});
export function sourceCanvas(info:VideoInfo):VideoProject["output"]|null{
  const [a,b]=(info.sample_aspect_ratio||"1:1").split(":").map(Number);
  const sar=Number.isFinite(a/b)&&a/b>0?a/b:1;
  let width=Math.floor(info.width*sar/2)*2,height=Math.floor(info.height/2)*2;
  if(Math.abs(Math.round(info.rotation_degrees/90))%2)[width,height]=[height,width];
  if(!Number.isFinite(width)||!Number.isFinite(height)||width<16||height<16||width>4096||height>4096||width*height>16000000||Math.max(width/height,height/width)>32)return null;
  return {width,height,fit:"fit"};
}
export function defaultOutput(info:VideoInfo):VideoProject["output"]{
  const portrait=(info.height>info.width)!==(Math.abs(Math.round(info.rotation_degrees/90))%2===1);
  return {width:portrait?1080:1920,height:portrait?1920:1080,fit:"fit"};
}
export function cropForAspect(info:VideoInfo,aspect:number):Crop{
  const [a,b]=(info.sample_aspect_ratio||"1:1").split(":").map(Number);
  const sar=Number.isFinite(a/b)&&a/b>0?a/b:1;
  let sourceAspect=info.width*sar/info.height;
  if(Math.abs(Math.round(info.rotation_degrees/90))%2)sourceAspect=1/sourceAspect;
  const width=Math.round(Math.min(1,aspect/sourceAspect)*10000),height=Math.round(Math.min(1,sourceAspect/aspect)*10000);
  return {x:Math.floor((10000-width)/2),y:Math.floor((10000-height)/2),width,height};
}
export function validProject(project:VideoProject):string {
  const clip=project.clips[0];if(!clip)return "A project needs one clip.";
  const start=markSeconds(clip.source_in),end=markSeconds(clip.source_out),crop=clip.edits.crop;
  const [a,b]=(project.asset.info.time_base||"1/1000000").split("/").map(Number);
  if(!project.title.trim()||project.title.length>200)return "Enter a project name of up to 200 characters.";
  if(!Number.isFinite(start)||!Number.isFinite(end)||start<0||end-start<0.04||end>project.asset.info.duration_seconds+a/b/2+0.000001)return "Keep at least 0.04 seconds, within the original duration.";
  if(![crop.x,crop.y,crop.width,crop.height].every(Number.isInteger)||crop.x<0||crop.y<0||crop.width<100||crop.height<100||crop.x+crop.width>10000||crop.y+crop.height>10000)return "The crop must fit inside the video and keep at least 1% of each dimension.";
  if(clip.edits.gain_percent<0||clip.edits.gain_percent>200||!Number.isInteger(clip.edits.gain_percent))return "Audio volume must be between 0 and 200%.";
  return "";
}
type History={past:VideoProject[];present:VideoProject;future:VideoProject[]};
export function editHistory(history:History,next:VideoProject|"undo"|"redo"):History {
  if(next==="undo")return history.past.length?{past:history.past.slice(0,-1),present:history.past.at(-1)!,future:[history.present,...history.future].slice(0,100)}:history;
  if(next==="redo")return history.future.length?{past:[...history.past,history.present].slice(-100),present:history.future[0],future:history.future.slice(1)}:history;
  if(JSON.stringify(next)===JSON.stringify(history.present))return history;
  return {past:[...history.past,history.present].slice(-100),present:next,future:[]};
}

const sessions=new Set<ProjectSession>();
const saveListeners=new Set<()=>void>();
export function subscribeProjectSaves(listener:()=>void){saveListeners.add(listener);return()=>{saveListeners.delete(listener);};}
export function projectSaveError(){return [...sessions].map(s=>s.error).find(Boolean)||"";}
// The queue outlives a player unmount. Each write uses the revision returned by its predecessor.
export class ProjectSession {
  history:History;status:"saved"|"saving"|"error"="saved";error="";revision:number;
  private queued:VideoProject|null=null;private running=false;private tail:Promise<void>=Promise.resolve();private released=false;
  private listeners=new Set<()=>void>();
  constructor(readonly root:string,loaded:LoadedProject){this.history={past:[],present:loaded.project,future:[]};this.revision=loaded.revision;sessions.add(this);}
  subscribe=(listener:()=>void)=>{this.listeners.add(listener);return()=>{this.listeners.delete(listener);};};
  private notify(){this.listeners.forEach(listener=>listener());if(this.released&&this.status==="saved")sessions.delete(this);saveListeners.forEach(listener=>listener());}
  commit(next:VideoProject|"undo"|"redo"){
    const history=editHistory(this.history,next);if(history===this.history)return;
    const error=validProject(history.present);if(error)throw Error(error);
    sessions.add(this);this.history=history;this.queued=history.present;this.pump();this.notify();
  }
  private pump(){
    if(this.running||this.error||!this.queued)return;
    const project=this.queued;this.queued=null;this.running=true;this.status="saving";
    this.tail=(async()=>{
      try{this.revision=await invoke<number>("save_video_project",{root:this.root,project,expectedRevision:this.revision});}
      catch(error){this.error=String(error);this.queued=this.history.present;}
      finally{this.running=false;this.status=this.error?"error":this.queued?"saving":"saved";this.notify();if(!this.error)this.pump();}
    })();
  }
  async flush(){while(this.running){await this.tail;}if(this.error)throw Error(this.error);if(this.queued){this.pump();await this.flush();}}
  retry(){this.error="";this.pump();this.notify();}
  release(){this.released=true;if(this.status==="saved")sessions.delete(this);}
  discard(){this.queued=null;this.error="";this.status="saved";sessions.delete(this);saveListeners.forEach(listener=>listener());}
}
export function hasUnsavedProjects(){return [...sessions].some(s=>s.status!=="saved");}
export async function flushProjects(retry=false){const active=[...sessions];if(retry)active.forEach(s=>s.retry());await Promise.all(active.map(s=>s.flush()));}
export function discardProjectSaves(){sessions.forEach(s=>s.discard());}
