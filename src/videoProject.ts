import {DurableProjectSession} from "./projectSession";
export {editHistory,subscribeProjectSaves,projectSaveError,hasUnsavedProjects,flushProjects,discardProjectSaves} from "./projectSession";
import {markSeconds,type VideoTime} from "./videoTime";
import type {VideoInfo} from "./VideoDetails";
export type Crop={x:number;y:number;width:number;height:number};
export type Edits={crop:Crop;quarter_turns:number;flip_horizontal:boolean;flip_vertical:boolean;muted:boolean;gain_percent:number;brightness:number;contrast:number};
export type VideoProject={schema_version:1;id:string;title:string;asset:{id:string;relative_path:string;sha256:string;bytes:number;info:VideoInfo};clips:{id:string;asset_id:string;source_in:VideoTime;source_out:VideoTime;edits:Edits}[];output:{width:number;height:number;fit:"fit"|"fill"}};
export type LoadedProject={project:VideoProject;revision:number;source_path:string|null;source_status:string};
export const defaultEdits=():Edits=>({crop:{x:0,y:0,width:10000,height:10000},quarter_turns:0,flip_horizontal:false,flip_vertical:false,muted:false,gain_percent:100,brightness:0,contrast:0});
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
  if(![clip.edits.brightness,clip.edits.contrast].every(value=>Number.isInteger(value)&&value>=-100&&value<=100))return "Brightness and contrast must be whole numbers from -100 to 100.";
  return "";
}
export class ProjectSession extends DurableProjectSession<VideoProject> {
  constructor(root:string,loaded:LoadedProject){super(root,loaded,"save_video_project",validProject);}
}
