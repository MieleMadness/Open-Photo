import {DurableProjectSession} from "./projectSession";
import {defaultEdits,validProject,type VideoProject} from "./videoProject";
import {markSeconds,markTime} from "./videoTime";
export type SequenceProject={schema_version:2;id:string;title:string;assets:VideoProject["asset"][];clips:VideoProject["clips"];output:VideoProject["output"];frame_rate:30|60};
export type LoadedSequence={project:SequenceProject;revision:number;sources:{asset_id:string;path:string|null;status:string}[]};
export function validSequence(project:SequenceProject){
  if(project.schema_version!==2||!project.assets.length||project.assets.length>50||project.clips.length>200||![30,60].includes(project.frame_rate))return "Choose 1–50 videos and at most 200 timeline clips.";
  for(const asset of project.assets){const error=validProject({...project,schema_version:1,asset,clips:[{id:"validation",asset_id:asset.id,source_in:markTime(0,asset.info.time_base),source_out:markTime(asset.info.duration_seconds,asset.info.time_base),edits:defaultEdits()}]});if(error)return error;}
  if(new Set(project.assets.map(a=>a.id)).size!==project.assets.length||new Set(project.clips.map(c=>c.id)).size!==project.clips.length)return "Project identities must be unique.";
  for(const clip of project.clips){const asset=project.assets.find(a=>a.id===clip.asset_id);if(!asset)return "A clip refers to a missing video.";const error=validProject({...project,schema_version:1,asset,clips:[clip]});if(error)return error;}
  if(project.clips.reduce((total,clip)=>total+markSeconds(clip.source_out)-markSeconds(clip.source_in),0)>7200)return "Keep this project within two hours.";
  return "";
}
export class SequenceSession extends DurableProjectSession<SequenceProject>{constructor(root:string,loaded:LoadedSequence){super(root,loaded,"save_video_sequence",validSequence);}}
