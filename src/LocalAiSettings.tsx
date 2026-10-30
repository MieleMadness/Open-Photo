type Model={kind:string;installed:boolean;compatible:boolean;bytes:number};
export function LocalAiSettings({enabled,onChange,models,busy,message,onRefresh,onCheck,checking,runtime,device,onDeviceChange}:{enabled:boolean;onChange:(value:boolean)=>void;models:Model[];busy:boolean;message:string;onRefresh:()=>void;onCheck:()=>void;checking:boolean;runtime:{directml_available:boolean}|null;device:"auto"|"cpu";onDeviceChange:(value:"auto"|"cpu")=>void}){
  const ready=models.length===7&&models.every(model=>model.compatible);
  return <section className="settings-card" aria-label="Local AI settings"><div>
    <h2>Local AI</h2><p>The AI models are included with Open-Photo. Turn AI on to use local search, People, and text recognition. Photos stay on this computer.</p>
    <label className="ai-switch"><input role="switch" aria-label="Local AI" type="checkbox" checked={enabled} disabled={!enabled&&(busy||!ready)} onChange={event=>onChange(event.target.checked)}/><strong>{enabled?"On":"Off"}</strong></label>
    <p>{busy?"Checking installed AI files…":ready?"AI files are installed and verified. No download is needed.":"AI files are missing or incompatible. Repair or reinstall Open-Photo to restore them."}</p>
    <small>Indexing starts when you choose an AI indexing action. Turning AI off stops further batches; a batch already running may finish. Your existing indexes and names are kept.</small>
    {message&&<p className="results-note" role="status">{message}</p>}
    <details><summary>AI diagnostics</summary><p>{runtime?.directml_available?"GPU acceleration available, with CPU fallback.":"Local CPU processing available."}</p><label>Processing<select aria-label="AI processing device" value={device} onChange={event=>onDeviceChange(event.target.value as "auto"|"cpu")}><option value="auto">Automatic</option><option value="cpu">CPU</option></select></label>{models.map(model=><p key={model.kind}>{model.kind}: {model.compatible?"Verified":model.installed?"Incompatible":"Missing"}</p>)}<button className="secondary-button" disabled={busy} onClick={onRefresh}>Check installed files</button><button className="secondary-button" disabled={checking} onClick={onCheck}>{checking?"Checking…":"Run AI self-check"}</button></details>
  </div></section>;
}
