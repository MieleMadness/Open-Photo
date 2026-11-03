import {DurableProjectSession} from "./projectSession";
import {defaultEdits,validProject,validEdits,type VideoProject} from "./videoProject";
import {markSeconds,markTime} from "./videoTime";
import {invoke} from "@tauri-apps/api/core";
import {assetTimeBase,assetDuration,videoInfo,type SequenceAsset} from "./sequenceMedia";
import {flushSettings} from "./userData";
export type SequenceProject={schema_version:3;id:string;title:string;assets:SequenceAsset[];clips:VideoProject["clips"];output:VideoProject["output"];frame_rate:30|60};
export type LoadedSequence={project:SequenceProject;revision:number;sources:{asset_id:string;path:string|null;root?:string|null;status:string}[]};
export function validSequence(project:SequenceProject){
  if(!project.title.trim()||project.title.length>200)return "Enter a project name of up to 200 characters.";
  if(project.schema_version!==3||!project.assets.length||project.assets.length>50||project.clips.length>200||![30,60].includes(project.frame_rate))return "Choose 1–50 media sources and at most 200 timeline clips.";
  if(new Set(project.assets.map(a=>a.id)).size!==project.assets.length||new Set(project.clips.map(c=>c.id)).size!==project.clips.length)return "Project identities must be unique.";
  function validateClip(asset:SequenceAsset,clip:VideoProject["clips"][number]){
    const info=videoInfo(asset);
    if(info)return validProject({...project,schema_version:1,asset:{...asset,info},clips:[clip]});
    const media=asset.media;if(media.kind!=="photo")return "Unsupported source type.";
    if(media.width<1||media.height<1||media.width>20000||media.height>20000||media.width*media.height>50000000)return "Unsupported photo dimensions.";
    const start=markSeconds(clip.source_in),end=markSeconds(clip.source_out);
    if(clip.source_in.time_base!=="1/1000000"||clip.source_out.time_base!=="1/1000000"||!Number.isFinite(start)||!Number.isFinite(end)||start<0||end-start<0.04||end>600)return "Photo clips must last 0.04–600 seconds.";
    return validEdits(clip.edits);
  }
  for(const asset of project.assets){const error=validateClip(asset,{id:"validation",asset_id:asset.id,source_in:markTime(0,assetTimeBase(asset)),source_out:markTime(assetDuration(asset),assetTimeBase(asset)),edits:defaultEdits()});if(error)return error;}
  for(const clip of project.clips){const asset=project.assets.find(a=>a.id===clip.asset_id);if(!asset)return "A clip refers to missing media.";const error=validateClip(asset,clip);if(error)return error;}
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
