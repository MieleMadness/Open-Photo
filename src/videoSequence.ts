import {validateCutTransitions,type CutTransition} from "./cutTransitions";
import {DurableProjectSession} from "./projectSession";
import {defaultEdits,validProject,validEdits,type VideoProject} from "./videoProject";
import {markSeconds,markTime} from "./videoTime";
import {invoke} from "@tauri-apps/api/core";
import {assetTimeBase,assetDuration,videoInfo,type SequenceAsset} from "./sequenceMedia";
import {flushSettings} from "./userData";
import {validTitleText} from "./titleText";
export type TitleOverlay={id:string;text:string;start_seconds:number;end_seconds:number;x_percent:number;y_percent:number};
export type AudioTrack={id:string;asset_id:string;start_seconds:number;source_in_seconds:number;duration_seconds:number;gain_percent:number;fade_in_seconds:number;fade_out_seconds:number};
export type AudioOnlySource={id:string;relative_path:string;sha256:string;bytes:number;info:{duration_seconds:number;stream:number;codec:string;channels:number;sample_rate:number}};
export type SequenceProject={transitions?:CutTransition[];titles?:TitleOverlay[];audio_tracks?:AudioTrack[];audio_sources?:AudioOnlySource[];schema_version:3;id:string;title:string;assets:SequenceAsset[];clips:VideoProject["clips"];output:VideoProject["output"];frame_rate:30|60};
export type LoadedSequence={project:SequenceProject;revision:number;sources:{asset_id:string;path:string|null;root?:string|null;status:string}[];transition_proposal?:boolean};
export function validSequence(project:SequenceProject){
  if(!project.title.trim()||project.title.length>200)return "Enter a project name of up to 200 characters.";
  const titles=project.titles||[];
  if(titles.length>20)return "Keep at most 20 titles.";
  const total=project.clips.reduce((sum,clip)=>sum+markSeconds(clip.source_out)-markSeconds(clip.source_in),0);
  const seen=new Set<string>();
  for(const title of titles){
    if(!/^[a-zA-Z0-9-]{1,120}$/.test(title.id)||seen.has(title.id)||!validTitleText(title.text)||!Number.isFinite(title.start_seconds)||!Number.isFinite(title.end_seconds)||title.start_seconds<0||title.end_seconds<=title.start_seconds||title.end_seconds>total+0.000001||!Number.isInteger(title.x_percent)||!Number.isInteger(title.y_percent)||title.x_percent<0||title.x_percent>100||title.y_percent<0||title.y_percent>100)return "Correct title timing, text or position.";
    seen.add(title.id);
  }
  const sources=project.audio_sources||[];
  if(sources.length>8)return "Keep at most eight independent audio sources.";
  if(new Set(sources.map(source=>source.id)).size!==sources.length)return "Independent audio source IDs must be unique.";
  if(sources.some(source=>project.assets.some(asset=>asset.id===source.id||(asset.bytes===source.bytes&&asset.sha256===source.sha256))))return "An audio source duplicates an existing media asset.";
  const tracks=project.audio_tracks||[];
  if(tracks.length>8)return "Keep at most eight audio tracks.";
  const trackIds=new Set<string>();
  for(const track of tracks){
    if(!/^[a-zA-Z0-9-]{1,120}$/.test(track.id)
      ||trackIds.has(track.id)
      ||!(project.audio_sources||[]).some(source=>source.id===track.asset_id&&track.source_in_seconds+track.duration_seconds<=source.info.duration_seconds+.000001)&&!project.assets.some(asset=>asset.id===track.asset_id&&asset.media.kind==="video"&&(asset.media.info.audio?.length||0)>0&&track.source_in_seconds+track.duration_seconds<=asset.media.info.duration_seconds+.000001)
      ||![track.start_seconds,track.source_in_seconds,track.duration_seconds,track.fade_in_seconds,track.fade_out_seconds].every(Number.isFinite)
      ||track.start_seconds<0||track.source_in_seconds<0||track.duration_seconds<.04
      ||track.start_seconds+track.duration_seconds>total+.000001
      ||!Number.isInteger(track.gain_percent)||track.gain_percent<0||track.gain_percent>200
      ||track.fade_in_seconds<0||track.fade_out_seconds<0
      ||track.fade_in_seconds+track.fade_out_seconds>track.duration_seconds)return "Correct audio track source, timing, gain or fades.";
    trackIds.add(track.id);
  }
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
  const transitionError=validateCutTransitions(project.clips.map(clip=>({id:clip.id,duration:markSeconds(clip.source_out)-markSeconds(clip.source_in)})),project.transitions||[],project.frame_rate);
  if(transitionError)return transitionError;
  if(project.clips.reduce((total,clip)=>total+markSeconds(clip.source_out)-markSeconds(clip.source_in),0)>7200)return "Keep this project within two hours.";
  return "";
}
export class SequenceSession extends DurableProjectSession<SequenceProject>{constructor(root:string,loaded:LoadedSequence,readonly workspaceId?:string){super(root,loaded,workspaceId?"save_workspace_video_project":"save_video_sequence",validSequence,workspaceId?{workspaceId}:undefined);}}
export type WorkspaceProjectAction=
  |{type:"open_source";path:string}
  |{type:"create";paths:string[]}|{type:"load";id:string}|{type:"import";path:string}
  |{type:"add";id:string;paths:string[];expected_revision:number}
  |{type:"add_audio";id:string;paths:string[];expected_revision:number}
  |{type:"remove_asset";id:string;asset_id:string;expected_revision:number}
  |{type:"relink";id:string;asset_id:string;path:string;expected_revision:number}
  |{type:"recover";id:string;history_id:number;expected_revision:number};
export async function workspaceProject(workspaceId:string,action:WorkspaceProjectAction){await flushSettings(["openphoto.workspaces"]);return invoke<LoadedSequence>("edit_workspace_video_project",{workspaceId,action});}
export async function workspaceProjects(workspaceId:string){await flushSettings(["openphoto.workspaces"]);return invoke<{projects:{id:string;title:string}[];warnings:string[]}>("workspace_video_projects",{workspaceId});}
