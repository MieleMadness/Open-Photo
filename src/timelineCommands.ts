import {defaultEdits,type VideoProject} from "./videoProject";
import {markSeconds,markTime} from "./videoTime";
import {validSequence,type SequenceProject} from "./videoSequence";

export type TimelineClip=VideoProject["clips"][number];
export type TimelineEntry={clip:TimelineClip;start:number;end:number;duration:number};
export type TimelineCommand=
  |{type:"move";clipId:string;index:number}
  |{type:"trim";clipId:string;start:number;end:number}
  |{type:"split";clipId:string;at:number;newId:string}
  |{type:"delete";clipId:string}
  |{type:"edits";clipId:string;edits:TimelineClip["edits"]}
  |{type:"reset";clipId:string}
  |{type:"append";assetId:string;newId:string};

/** Sequence order owns placement. Linked source audio is part of each clip's edits. */
export function timelineEntries(project:SequenceProject):TimelineEntry[]{
  let position=0;
  return project.clips.map(clip=>{const duration=markSeconds(clip.source_out)-markSeconds(clip.source_in),start=position;position+=duration;return {clip,start,end:position,duration};});
}
export function projectDuration(project:SequenceProject):number{return timelineEntries(project).at(-1)?.end||0;}

/** Pure, atomic commands; callers commit the returned snapshot once to the shared save/history queue. */
export function editTimeline(project:SequenceProject,command:TimelineCommand):SequenceProject{
  const next=structuredClone(project);
  if(command.type==="append"){
    const asset=next.assets.find(a=>a.id===command.assetId);if(!asset)throw Error("Choose a video from this project's media bin.");
    next.clips.push({id:command.newId,asset_id:asset.id,source_in:markTime(0,asset.info.time_base),source_out:markTime(asset.info.duration_seconds,asset.info.time_base),edits:defaultEdits()});
  }else{
    const index=next.clips.findIndex(c=>c.id===command.clipId);if(index<0)throw Error("The selected clip is no longer in this project.");
    const clip=next.clips[index],asset=next.assets.find(a=>a.id===clip.asset_id)!;
    if(command.type==="move"){
      if(!Number.isInteger(command.index)||command.index<0||command.index>=next.clips.length)throw Error("Choose a position inside the timeline.");
      next.clips.splice(index,1);next.clips.splice(command.index,0,clip);
    }else if(command.type==="delete"){
      next.clips.splice(index,1);
    }else if(command.type==="trim"){
      if(!Number.isFinite(command.start)||!Number.isFinite(command.end))throw Error("Enter valid trim times.");
      clip.source_in=markTime(command.start,asset.info.time_base);clip.source_out=markTime(command.end,asset.info.time_base);
      if(command.start<0||command.end<=command.start)throw Error("Trim end must be after trim start.");
    }else if(command.type==="edits"){
      clip.edits=structuredClone(command.edits);
    }else if(command.type==="reset"){
      clip.edits=defaultEdits();clip.source_in=markTime(0,asset.info.time_base);clip.source_out=markTime(asset.info.duration_seconds,asset.info.time_base);
    }else{
      const entry=timelineEntries(project)[index];
      if(!Number.isFinite(command.at)||command.at<=entry.start||command.at>=entry.end)throw Error("Place the playhead inside the selected clip to split it.");
      const cut=markTime(markSeconds(clip.source_in)+command.at-entry.start,asset.info.time_base);
      const right={...structuredClone(clip),id:command.newId,source_in:cut};clip.source_out=cut;
      next.clips.splice(index+1,0,right);
    }
  }
  const error=validSequence(next);if(error)throw Error(error);
  return next;
}
