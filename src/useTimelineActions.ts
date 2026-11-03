import {useState} from "react";
import {editTimeline,type TimelineCommand} from "./timelineCommands";
import type {SequenceSession} from "./videoSequence";

/** Commands commit to one revision-safe history regardless of the panel invoking them. */
export function useTimelineActions(session:SequenceSession,selected:string,onSelected:(id:string)=>void){
  const [error,setError]=useState("");
  function history(action:"undo"|"redo"){session.commit(action);const clips=session.history.present.clips;onSelected(clips.find(c=>c.id===selected)?.id||clips[0]?.id||"");}
  function command(action:TimelineCommand){
    try{
      const index=session.history.present.clips.findIndex(c=>c.id===selected);
      const next=editTimeline(session.history.present,action);session.commit(next);setError("");
      if(action.type==="append")onSelected(action.newId);
      else if(action.type==="delete")onSelected(next.clips[Math.min(index,next.clips.length-1)]?.id||"");
      return "";
    }catch(cause){const message=String(cause);setError(message);return message;}
  }
  return {command,history,error};
}
