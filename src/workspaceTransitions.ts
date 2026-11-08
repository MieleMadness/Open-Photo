/** Native-owned transition proposals; the existing project session owns saves and identity. */
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
import {type SequenceSession,type LoadedSequence} from "./videoSequence";
import {type CrossfadeBoundary} from "./crossfadeClock";

export type ProjectReceipt={revision:number;hash:string};
export type TransitionRecord={schema_version:1;timing_policy:1;workspace:string;project_id:string;revision:number;project:ProjectReceipt;edges:CrossfadeBoundary[];recovered_from:number|null};
type Span={start:number;end:number};
export type LayerPlan={fps:30|60;total_frames:number;total_samples:number;clips:{id:string;asset_id:string;photo:boolean;linked_audio:boolean;gain_percent:number;frames:Span;samples:Span;dip_in_frames:number;dip_out_frames:number}[];titles:{id:string;frames:Span}[];audio:{id:string;asset_id:string;samples:Span;source_samples:Span;fade_in_samples:number;fade_out_samples:number;gain_percent:number}[]};
export type FrameSeek={project_frame:number;fps:30|60;samples:Span;video:{clip_id:string;asset_id:string;photo:boolean;part_frame:number;weight:{numerator:number;denominator:number};dip_in_frames:number;dip_out_frames:number}[];titles:string[];audio:{id:string;asset_id:string;origin:"normalized_clip"|"added_track";samples:Span;source_sample_start:number;silence:boolean;gain_percent:number;fade_in:Span|null;fade_out:Span|null}[]};
export type TransitionState={project:ProjectReceipt;revision:number;record:TransitionRecord|null;stale:boolean;history:{revision:number;supported:boolean}[];timing:LayerPlan|null;seek:FrameSeek|null;portable:string|null};
export type SeekPoint={type:"frame";value:number}|{type:"seconds";value:number};
export type TransitionRenderRequest={workspace_id:string;id:string;expected_project:ProjectReceipt;expected_revision:number};
export type TransitionCapabilities={rendering:boolean;reason:string|null};
export async function transitionCapabilities():Promise<TransitionCapabilities>{
  const value=await invoke<TransitionCapabilities>("workspace_transition_capabilities");
  if(!value||typeof value.rendering!=="boolean"||(value.reason!==null&&typeof value.reason!=="string"))throw Error("Crossfade availability could not be checked. Retry opening the proposal.");
  return value;
}
type Action=
  |{type:"import_file";id:string;expected_project:ProjectReceipt;expected_revision:number;path:string;rebase:boolean}
  |{type:"export";id:string;expected_project:ProjectReceipt;expected_revision:number}
  |{type:"export_history";id:string;expected_project:ProjectReceipt;expected_revision:number;history_revision:number}
  |{type:"import";id:string;expected_project:ProjectReceipt;expected_revision:number;document:string;rebase:boolean}
  |{type:"load";id:string}
  |{type:"save";id:string;expected_project:ProjectReceipt;expected_revision:number;edges:CrossfadeBoundary[];rebase:boolean}
  |{type:"recover";id:string;expected_project:ProjectReceipt;expected_revision:number;history_revision:number;rebase:boolean}
  |{type:"history";id:string;before_revision:number|null;limit:number}
  |{type:"seek";id:string;expected_project:ProjectReceipt;expected_revision:number;point:SeekPoint};
const safe=(n:unknown,min=0):n is number=>typeof n==="number"&&Number.isSafeInteger(n)&&n>=min;
function receipt(value:unknown):value is ProjectReceipt{
  const r=value as ProjectReceipt|null;
  return !!r&&safe(r.revision,1)&&typeof r.hash==="string"&&/^[a-f0-9]{64}$/i.test(r.hash);
}
function checkState(value:unknown,workspace:string,id:string):asserts value is TransitionState{
  const s=value as TransitionState|null,r=s?.record;
  if(!s||!receipt(s.project)||!safe(s.revision)||typeof s.stale!=="boolean"||!Array.isArray(s.history)
    ||s.history.length>100||s.history.some(h=>!safe(h.revision,1)||typeof h.supported!=="boolean")
    ||(r===null?s.revision!==0||s.stale:!r||r.schema_version!==1||r.timing_policy!==1||r.workspace!==workspace||r.project_id!==id||r.revision!==s.revision||!safe(r.revision,1)||!receipt(r.project)||!Array.isArray(r.edges)||r.edges.length>199
      ||r.edges.some(e=>!e||typeof e.from_clip_id!=="string"||typeof e.to_clip_id!=="string"||!safe(e.frames,1))
      ||s.stale!==(r.project.revision!==s.project.revision||r.project.hash!==s.project.hash))
    ||(s.stale?s.timing!==null:s.timing===null?!!r?.edges.length:!s.timing||![30,60].includes(s.timing.fps)||!safe(s.timing.total_frames,1)||!safe(s.timing.total_samples,1)))
    throw Error("Unsupported or inconsistent transition draft. Preserve the stored data and reload with a compatible version.");
}
function owner(session:SequenceSession){
  if(!session.workspaceId)throw Error("Transition drafts require a saved workspace project.");
  return {workspace:session.workspaceId,id:session.history.present.id};
}
async function request(session:SequenceSession,action:Action):Promise<TransitionState>{
  const {workspace,id}=owner(session);
  await flushSettings(["openphoto.workspaces"]);
  await session.flush();
  const project=session.history.present,revision=session.revision;
  if(project.id!==id)throw Error("Project changed before the transition request.");
  const result:unknown=await invoke("edit_workspace_video_transitions",{workspaceId:workspace,action});
  if(session.history.present!==project||session.revision!==revision)throw Error("Project changed during the transition request. Reload the draft.");
  checkState(result,workspace,id);
  if(result.project.revision!==revision)throw Error("The saved project changed elsewhere. Reload the project before editing transitions.");
  return result;
}
function expected(session:SequenceSession,state:TransitionState){
  const {workspace,id}=owner(session);checkState(state,workspace,id);
  return {id,expected_project:{...state.project},expected_revision:state.revision};
}
export function transitionRenderRequest(session:SequenceSession,state:TransitionState):TransitionRenderRequest{
  const expectedState=expected(session,state);
  if(state.stale||state.project.revision!==session.revision||!state.record?.edges.length||!state.timing)throw Error("Save and rebase the current crossfade proposal before rendering it.");
  return {workspace_id:owner(session).workspace,...expectedState};
}
export function loadTransitionDraft(session:SequenceSession){return request(session,{type:"load",id:owner(session).id});}
export function saveTransitionDraft(session:SequenceSession,state:TransitionState,edges:CrossfadeBoundary[],rebase=false){
  return request(session,{type:"save",...expected(session,state),edges:edges.map(e=>({...e})),rebase});
}
export function recoverTransitionDraft(session:SequenceSession,state:TransitionState,historyRevision:number,rebase=false){
  return request(session,{type:"recover",...expected(session,state),history_revision:historyRevision,rebase});
}
export function transitionHistory(session:SequenceSession,beforeRevision:number|null=null,limit=25){
  return request(session,{type:"history",id:owner(session).id,before_revision:beforeRevision,limit});
}
export function seekTransitionDraft(session:SequenceSession,state:TransitionState,point:SeekPoint){
  return request(session,{type:"seek",...expected(session,state),point:{...point}});
}
export function exportTransitionDraft(session:SequenceSession,state:TransitionState){
  return request(session,{type:"export",...expected(session,state)});
}
export function importTransitionDraft(session:SequenceSession,state:TransitionState,document:string,rebase=false){
  return request(session,{type:"import",...expected(session,state),document,rebase});
}
export function importTransitionFile(session:SequenceSession,state:TransitionState,path:string,rebase=false){
  return request(session,{type:"import_file",...expected(session,state),path,rebase});
}
export function writeTransitionCopy(document:string,destination:string){
  return invoke<void>("write_transition_proposal_copy",{document,destination});
}
export function exportTransitionHistory(session:SequenceSession,state:TransitionState,historyRevision:number){
  return request(session,{type:"export_history",...expected(session,state),history_revision:historyRevision});
}
export async function recoverTransitionProject(session:SequenceSession,state:TransitionState,historyRevision:number){
  const expectedState=expected(session,state),workspace=owner(session).workspace;
  await flushSettings(["openphoto.workspaces"]);
  await session.flush();
  return invoke<LoadedSequence>("recover_workspace_transition_project",{request:{workspace_id:workspace,...expectedState},historyRevision});
}
