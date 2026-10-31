import {invoke} from "@tauri-apps/api/core";
type Participant={status:"saved"|"saving"|"error";error:string;flush:()=>Promise<void>;retry:()=>void;discard:()=>void};
const sessions=new Set<Participant>();
const listeners=new Set<()=>void>();
export function subscribeProjectSaves(listener:()=>void){listeners.add(listener);return()=>{listeners.delete(listener);};}
export function projectSaveError(){return [...sessions].map(s=>s.error).find(Boolean)||"";}
export function hasUnsavedProjects(){return [...sessions].some(s=>s.status!=="saved");}
export async function flushProjects(retry=false){const active=[...sessions];if(retry)active.forEach(s=>s.retry());await Promise.all(active.map(s=>s.flush()));}
export function discardProjectSaves(){sessions.forEach(s=>s.discard());}
export type History<P extends object>={past:P[];present:P;future:P[]};
export function editHistory<P extends object>(h:History<P>,next:P|"undo"|"redo"):History<P>{
  if(next==="undo")return h.past.length?{past:h.past.slice(0,-1),present:h.past.at(-1)!,future:[h.present,...h.future].slice(0,100)}:h;
  if(next==="redo")return h.future.length?{past:[...h.past,h.present].slice(-100),present:h.future[0],future:h.future.slice(1)}:h;
  return JSON.stringify(next)===JSON.stringify(h.present)?h:{past:[...h.past,h.present].slice(-100),present:next as P,future:[]};
}
// Both project formats share the revision queue and the app-close/updater save guard.
export class DurableProjectSession<P extends object> {
  history:History<P>;status:"saved"|"saving"|"error"="saved";error="";revision:number;
  private queued:P|null=null;private running=false;private tail:Promise<void>=Promise.resolve();private released=false;
  private observers=new Set<()=>void>();
  constructor(readonly root:string,loaded:{project:P;revision:number},private command:string,private validate:(p:P)=>string,private scope?:Record<string,unknown>){this.history={past:[],present:loaded.project,future:[]};this.revision=loaded.revision;sessions.add(this);}
  subscribe=(listener:()=>void)=>{this.observers.add(listener);return()=>{this.observers.delete(listener);};};
  private notify(){this.observers.forEach(listener=>listener());if(this.released&&this.status==="saved")sessions.delete(this);listeners.forEach(listener=>listener());}
  commit(next:P|"undo"|"redo"){
    const h=this.history,history=editHistory(h,next);
    if(history===h)return;const error=this.validate(history.present);if(error)throw Error(error);
    sessions.add(this);this.history=history;this.queued=history.present;this.pump();this.notify();
  }
  private pump(){
    if(this.running||this.error||!this.queued)return;
    const project=this.queued;this.queued=null;this.running=true;this.status="saving";
    this.tail=(async()=>{
      try{this.revision=await invoke<number>(this.command,{...(this.scope||{root:this.root}),project,expectedRevision:this.revision});}
      catch(error){this.error=String(error);this.queued=this.history.present;}
      finally{this.running=false;this.status=this.error?"error":this.queued?"saving":"saved";this.notify();if(!this.error)this.pump();}
    })();
  }
  async flush(){while(this.running){await this.tail;}if(this.error)throw Error(this.error);if(this.queued){this.pump();await this.flush();}}
  retry(){this.error="";this.pump();this.notify();}
  release(){this.released=true;if(this.status==="saved")sessions.delete(this);}
  discard(){this.queued=null;this.error="";this.status="saved";sessions.delete(this);listeners.forEach(listener=>listener());}
}
