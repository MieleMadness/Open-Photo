import {DurableProjectSession} from "./projectSession";
import {validProject,type VideoProject} from "./videoProject";
export type SequenceProject={schema_version:2;id:string;title:string;assets:VideoProject["asset"][];clips:VideoProject["clips"];output:VideoProject["output"];frame_rate:30|60};
export type LoadedSequence={project:SequenceProject;revision:number;sources:{asset_id:string;path:string|null;status:string}[]};
export function validSequence(project:SequenceProject){
  if(project.schema_version!==2||!project.assets.length||project.assets.length>50||!project.clips.length||project.clips.length>200||![30,60].includes(project.frame_rate))return "Choose 1–50 videos for the project.";
  if(new Set(project.assets.map(a=>a.id)).size!==project.assets.length||new Set(project.clips.map(c=>c.id)).size!==project.clips.length)return "Project identities must be unique.";
  for(const clip of project.clips){const asset=project.assets.find(a=>a.id===clip.asset_id);if(!asset)return "A clip refers to a missing video.";const error=validProject({...project,schema_version:1,asset,clips:[clip]});if(error)return error;}
  return "";
}
export class SequenceSession extends DurableProjectSession<SequenceProject>{constructor(root:string,loaded:LoadedSequence){super(root,loaded,"save_video_sequence",validSequence);}}
