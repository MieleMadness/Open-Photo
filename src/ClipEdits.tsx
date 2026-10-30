import type {Geometry} from "./sequenceMedia";
import {NumberField} from "./EditorFields";
import {cropForAspect,type Edits} from "./videoProject";

/** One selected-clip inspector for one-source and multi-source projects. */
export function ClipEdits({edits,info,onChange,photo=false}:{edits:Edits;info:Geometry;photo?:boolean;onChange:(mutate:(edit:Edits)=>void)=>string}){
  return <div className="clip-edits">
    <details><summary>{photo?"Transform":"Transform and audio"}</summary><div className="video-edit-fields">
      <label>Rotation<select aria-label={photo?"Photo rotation":"Video rotation"} value={edits.quarter_turns} onChange={e=>onChange(edit=>{edit.quarter_turns=Number(e.target.value);})}>{[0,1,2,3].map(turn=><option key={turn} value={turn}>{turn*90}° clockwise</option>)}</select></label>
      <label><input type="checkbox" checked={edits.flip_horizontal} onChange={e=>onChange(edit=>{edit.flip_horizontal=e.target.checked;})}/>Flip horizontally</label>
      <label><input type="checkbox" checked={edits.flip_vertical} onChange={e=>onChange(edit=>{edit.flip_vertical=e.target.checked;})}/>Flip vertically</label>
      {!photo&&<>
      <label><input type="checkbox" checked={edits.muted} onChange={e=>onChange(edit=>{edit.muted=e.target.checked;})}/>Mute video audio</label>
      <NumberField label="Audio volume (%)" value={edits.gain_percent} min={0} max={200} onCommit={value=>onChange(edit=>{edit.gain_percent=value;})}/>
      </>}
    </div></details>
    <details open><summary>Crop & image adjustments</summary><div className="video-edit-fields">
      <NumberField label={photo?"Photo brightness":"Video brightness"} value={edits.brightness} min={-100} max={100} onCommit={value=>onChange(edit=>{edit.brightness=value;})}/>
      <NumberField label={photo?"Photo contrast":"Video contrast"} value={edits.contrast} min={-100} max={100} onCommit={value=>onChange(edit=>{edit.contrast=value;})}/>
      <button className="secondary-button" onClick={()=>onChange(edit=>{edit.brightness=0;edit.contrast=0;})}>Reset brightness and contrast</button>
      {(["x","y","width","height"] as const).map(field=><NumberField key={field} label={`Crop ${field} (%)`} value={edits.crop[field]/100} min={field==="width"||field==="height"?1:0} max={100} step={0.01} onCommit={value=>onChange(edit=>{edit.crop[field]=Math.round(value*100);})}/>)}
    </div><div className="video-transport"><button className="secondary-button" onClick={()=>onChange(edit=>{edit.crop={x:0,y:0,width:10000,height:10000};})}>Full source</button><button className="secondary-button" onClick={()=>onChange(edit=>{edit.crop={x:2500,y:2500,width:5000,height:5000};})}>Center half</button>{[["Square crop",1],["Landscape crop",16/9],["Portrait crop",9/16]].map(([label,aspect])=><button className="secondary-button" key={label} onClick={()=>onChange(edit=>{edit.crop=cropForAspect(info,Number(aspect));})}>{label}</button>)}</div></details>
    <p>These settings apply to the selected clip. Video clips keep their linked original audio. Original-source playback does not show saved edits.</p>
  </div>;
}
