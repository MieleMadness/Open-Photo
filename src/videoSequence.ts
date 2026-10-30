import {DurableProjectSession} from "./projectSession";
import {defaultEdits,validProject,type VideoProject} from "./videoProject";
import {markSeconds,markTime} from "./videoTime";
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
export type SequenceProject={schema_version:2;id:string;title:string;assets:VideoProject["asset"][];clips:VideoProject["clips"];output:VideoProject["output"];frame_rate:30|60};
export type LoadedSequence={project:SequenceProject;revision:number;sources:{asset_id:string;path:string|null;root?:string|null;status:string}[]};
export function validSequence(project:SequenceProject){
  if(project.schema_version!==2||!project.assets.length||project.assets.length>50||project.clips.length>200||![30,60].includes(project.frame_rate))return "Choose 1–50 videos and at most 200 timeline clips.";
  for(const asset of project.assets){const error=validProject({...project,schema_version:1,asset,clips:[{id:"validation",asset_id:asset.id,source_in:markTime(0,asset.info.time_base),source_out:markTime(asset.info.duration_seconds,asset.info.time_base),edits:defaultEdits()}]});if(error)return error;}
  if(new Set(project.assets.map(a=>a.id)).size!==project.assets.length||new Set(project.clips.map(c=>c.id)).size!==project.clips.length)return "Project identities must be unique.";
  for(const clip of project.clips){const asset=project.assets.find(a=>a.id===clip.asset_id);if(!asset)return "A clip refers to a missing video.";const error=validProject({...project,schema_version:1,asset,clips:[clip]});if(error)return error;}
  if(project.clips.reduce((total,clip)=>total+markSeconds(clip.source_out)-markSeconds(clip.source_in),0)>7200)return "Keep this project within two hours.";
  return "";
}
export class SequenceSession extends DurableProjectSession<SequenceProject>{constructor(root:string,loaded:LoadedSequence,readonly workspaceId?:string){super(root,loaded,workspaceId?"save_workspace_video_project":"save_video_sequence",validSequence,workspaceId?{workspaceId}:undefined);}}
export type WorkspaceProjectAction=
  |{type:"open_source";path:string}
  |{type:"create";paths:string[]}|{type:"load";id:string}|{type:"import";path:string}
  |{type:"add";id:string;paths:string[];expected_revision:number}
  |{type:"remove_asset";id:string;asset_id:string;expected_revision:number}
  |{type:"relink";id:string;asset_id:string;path:string;expected_revision:number}
  |{type:"recover";id:string;history_id:number;expected_revision:number};
export async function workspaceProject(workspaceId:string,action:WorkspaceProjectAction){await flushSettings(["openphoto.workspaces"]);return invoke<LoadedSequence>("edit_workspace_video_project",{workspaceId,action});}
export async function workspaceProjects(workspaceId:string){await flushSettings(["openphoto.workspaces"]);return invoke<{projects:{id:string;title:string}[];warnings:string[]}>("workspace_video_projects",{workspaceId});}
/** Compatibility adapter for the proven one-source render pipeline, not a separate editor. */
export function singleRenderProject(project:SequenceProject):VideoProject|null{
  if(project.clips.length!==1)return null;const clip=project.clips[0],asset=project.assets.find(a=>a.id===clip.asset_id);return asset?{schema_version:1,id:project.id,title:project.title,asset,clips:[clip],output:project.output}:null;
}
