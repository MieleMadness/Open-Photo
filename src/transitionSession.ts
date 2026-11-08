import {DurableProjectSession} from "./projectSession";
import {type CrossfadeBoundary} from "./crossfadeClock";
import {compileMarkedCrossfadeClock} from "./crossfadeProjectClock";
import {type SequenceSession,type SequenceProject} from "./videoSequence";
import {saveTransitionDraft,type TransitionState} from "./workspaceTransitions";
export type TransitionEdits={edges:CrossfadeBoundary[]};
export function maximumCrossfadeFrames(project:SequenceProject,edges:CrossfadeBoundary[],pair:number):number {
  if(!project.clips[pair+1])return 0;
  try{
    const source=compileMarkedCrossfadeClock(project.clips,project.frame_rate,[]);
    const free=(index:number)=>{const id=source.entries[index].id;
      const other=edges.filter(e=>!(e.from_clip_id===source.entries[pair].id&&e.to_clip_id===source.entries[pair+1].id)).filter(e=>e.from_clip_id===id||e.to_clip_id===id).reduce((sum,e)=>sum+e.frames,0);
      const dips=(project.transitions||[]).filter(e=>e.from_clip_id===id||e.to_clip_id===id).reduce((sum,e)=>sum+Math.ceil(e.duration_seconds*project.frame_rate/2),0);
      return source.entries[index].frames-other-dips-2;
    };
    return Math.max(0,Math.min(free(pair),free(pair+1)));
  }catch{return 0;}
}
export function validateTransitionEdits(project:SequenceProject,edges:CrossfadeBoundary[]):string {
  try{
    if(!edges.length)return "";
    const clock=compileMarkedCrossfadeClock(project.clips,project.frame_rate,edges);
    for(const transition of project.transitions||[])if(edges.some(e=>e.from_clip_id===transition.from_clip_id&&e.to_clip_id===transition.to_clip_id))return "Remove the dip at this boundary before choosing a crossfade.";
    for(let index=0;index<clock.entries.length;index++){
      const clip=clock.entries[index];
      const incoming=edges.find(e=>e.to_clip_id===clip.id)?.frames||0,outgoing=edges.find(e=>e.from_clip_id===clip.id)?.frames||0;
      const dipIn=Math.ceil(((project.transitions||[]).find(e=>e.to_clip_id===clip.id)?.duration_seconds||0)*project.frame_rate/2);
      const dipOut=Math.ceil(((project.transitions||[]).find(e=>e.from_clip_id===clip.id)?.duration_seconds||0)*project.frame_rate/2);
      if(incoming+outgoing+dipIn+dipOut+2>clip.frames)return "Keep two clear frames between transition envelopes.";
    }
    if((project.titles||[]).some(t=>Math.round(t.end_seconds*project.frame_rate)>clock.total_frames))return "A title extends past the shortened project. Edit its range before saving crossfades.";
    const samples=clock.total_frames*48000/project.frame_rate;
    if((project.audio_tracks||[]).some(t=>Math.round((t.start_seconds+t.duration_seconds)*48000)>samples))return "An audio track extends past the shortened project. Edit its range before saving crossfades.";
    return "";
  }catch(error){return String(error);}
}
/** Reuses the application's revision queue, undo/redo and close/update save guard. */
export class TransitionSession extends DurableProjectSession<TransitionEdits>{
  readonly sourceProject:SequenceProject;
  readonly source:SequenceSession;
  private native:()=>TransitionState;
  constructor(source:SequenceSession,initial:TransitionState){
    let latest=initial;const base=source.history.present;
    super(source.root,{project:{edges:(initial.record?.edges||[]).map(e=>({...e}))},revision:initial.revision},async(edits,expected)=>{
      if(latest.revision!==expected)throw Error("Transition receipts changed. Reload the saved proposal.");
      latest=await saveTransitionDraft(source,latest,edits.edges);
      return latest.revision;
    },edits=>{
      if(initial.stale||source.history.present!==base||source.revision!==latest.project.revision)return "The source timeline changed. Reload and explicitly rebase the proposal.";
      return validateTransitionEdits(base,edits.edges);
    });
    this.source=source;this.sourceProject=base;this.native=()=>latest;
  }
  get saved(){return this.native();}
  get stale(){return this.saved.stale||this.source.history.present!==this.sourceProject||this.source.revision!==this.saved.project.revision;}
}
