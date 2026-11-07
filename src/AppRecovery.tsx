import {Component,useState,type ReactNode} from "react";
import {SettingsRepairError,repairStructuredSetting} from "./settingsRepository";
import {ErrorNotice} from "./Ui";

function Recovery({error,onRetry}:{error:unknown;onRetry:()=>void}){
  const repair=error instanceof SettingsRepairError?error:null;
  const [draft,setDraft]=useState(repair?.original||""),[failure,setFailure]=useState(""),[busy,setBusy]=useState(false);
  function download(){if(!repair)return;const url=URL.createObjectURL(new Blob([repair.original],{type:"application/json"}));const link=document.createElement("a");link.href=url;link.download=`open-photo-${repair.key}-original.json`;link.click();setTimeout(()=>URL.revokeObjectURL(url),0);}
  return <main className="startup-recovery"><h1>Open-Photo</h1><ErrorNotice title="Your saved library could not be opened" error={error}/><p>Your saved data has been preserved. Original photos and videos have not been changed.</p>{repair&&<section><h2>Repair saved {repair.key}</h2><p>Save the original value before making a repair. Correct the JSON below, then apply it. Unsupported future versions must be opened in a compatible version of Open-Photo.</p><button className="secondary-button" onClick={download}>Save original settings</button><label className="settings-repair-field">Saved settings JSON<textarea spellCheck={false} value={draft} onChange={event=>setDraft(event.target.value)}/></label>{failure&&<p role="alert">{failure}</p>}<button className="primary-button" disabled={busy} onClick={()=>{setBusy(true);setFailure("");void repairStructuredSetting(repair.key,draft).then(onRetry).catch(error=>setFailure(String(error))).finally(()=>setBusy(false));}}>{busy?"Saving repair…":"Apply repaired settings"}</button></section>}<button className="secondary-button" onClick={onRetry}>Retry opening</button></main>;
}
export class AppRecovery extends Component<{children:ReactNode},{error:unknown;generation:number}>{
  state={error:null as unknown,generation:0};
  static getDerivedStateFromError(error:unknown){return {error};}
  render(){return this.state.error?<Recovery key={this.state.generation} error={this.state.error} onRetry={()=>this.setState(state=>({error:null,generation:state.generation+1}))}/>:this.props.children;}
}
