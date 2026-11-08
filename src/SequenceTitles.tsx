import {useEffect,useId,useRef,useState} from "react";
import {validTitleText} from "./titleText";
import {type SequenceSession,type TitleOverlay} from "./videoSequence";
import {projectDuration} from "./timelineCommands";

/** Edit locally until blur so temporarily blank text does not invalidate a saved project. */
function TitleTextInput({title,onCommit}:{title:TitleOverlay;onCommit:(text:string)=>void}){
  const [draft,setDraft]=useState(title.text);
  const cancelled=useRef(false);
  const errorId=useId(),invalid=!validTitleText(draft);
  useEffect(()=>setDraft(title.text),[title.id,title.text]);
  function finish(){
    if(cancelled.current){cancelled.current=false;return;}
    if(validTitleText(draft))onCommit(draft);
  }
  return <><input aria-label="Title text" value={draft} maxLength={200} required aria-invalid={invalid||undefined} aria-describedby={invalid?errorId:undefined} onFocus={()=>{cancelled.current=false;}} onChange={event=>setDraft(event.target.value)} onBlur={finish} onKeyDown={event=>{if(event.nativeEvent.isComposing)return;if(event.key==="Enter"){event.preventDefault();event.currentTarget.blur();}if(event.key==="Escape"){event.preventDefault();cancelled.current=true;setDraft(title.text);event.currentTarget.blur();}}}/>{invalid&&<span id={errorId} role="status">Use 1–100 characters without control characters. Press Escape to cancel.</span>}</>;
}

/** Simple, always-visible controls; timeline title range is measured in project seconds. */
export function SequenceTitles({session,disabled,playhead}:{session:SequenceSession;disabled:boolean;playhead:number}){
  const project=session.history.present;
  const titles=project.titles||[];
  const duration=projectDuration(project);
  function commit(next:TitleOverlay[]){session.commit({...project,titles:next});}
  function update(id:string,change:Partial<TitleOverlay>){
    commit(titles.map(title=>title.id===id?{...title,...change}:title));
  }
  return <section aria-label="Video text overlays" className="sequence-titles">
    <div className="timeline-heading"><h2>Titles</h2><p>{titles.length} / 20</p></div>
    <p className="timeline-help">Add a title at the playhead. Titles appear in the rendered project preview and export, not the original-source preview.</p>
    <fieldset disabled={disabled||duration<=0}>
      <button className="secondary-button" type="button" disabled={titles.length>=20||duration<=0} onClick={()=>{
        const start=Math.min(Math.max(0,playhead),Math.max(0,duration-0.04));
        const end=Math.min(duration,start+3);
        commit([...titles,{id:crypto.randomUUID(),text:"Your title",start_seconds:start,end_seconds:end,x_percent:50,y_percent:50}]);
      }}>Add title</button>
      {titles.map(title=><div key={title.id} className="sequence-title-fields">
        <label>Title text<TitleTextInput title={title} onCommit={text=>update(title.id,{text})}/></label>
        <label>Start (seconds)<input aria-label="Title start" type="number" min={0} max={Math.max(0,title.end_seconds-.01)} step=".01" value={title.start_seconds} onChange={event=>{const value=Number(event.target.value);if(Number.isFinite(value)&&value>=0&&value<title.end_seconds)update(title.id,{start_seconds:value});}}/></label>
        <label>End (seconds)<input aria-label="Title end" type="number" min={title.start_seconds+.01} max={duration} step=".01" value={title.end_seconds} onChange={event=>{const value=Number(event.target.value);if(Number.isFinite(value)&&value>title.start_seconds&&value<=duration)update(title.id,{end_seconds:value});}}/></label>
        <label>Horizontal placement<select aria-label="Title horizontal position" value={title.x_percent} onChange={event=>update(title.id,{x_percent:Number(event.target.value)})}><option value={10}>Left</option><option value={50}>Center</option><option value={90}>Right</option></select></label>
        <label>Vertical placement<select aria-label="Title vertical position" value={title.y_percent} onChange={event=>update(title.id,{y_percent:Number(event.target.value)})}><option value={10}>Top</option><option value={50}>Middle</option><option value={90}>Bottom</option></select></label>
        <button className="secondary-button" type="button" aria-label="Remove title" onClick={()=>commit(titles.filter(item=>item.id!==title.id))}>Remove title</button>
      </div>)}
    </fieldset>
  </section>;
}
