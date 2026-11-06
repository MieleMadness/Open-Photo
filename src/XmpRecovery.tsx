import {useEffect,useRef,useState} from "react";
import {invoke} from "@tauri-apps/api/core";
type Review={id:number;path:string;state:string;original:string|null;candidate:string;candidate_hash:string;original_hash:string|null;integrity_error?:string|null;backup_name?:string|null};
type ReviewSummary=Pick<Review,"id"|"path"|"state"|"candidate_hash">;
type ReviewPage={items:ReviewSummary[];next_before:number|null};
type StagingInventory={entries:{name:string;kind:string;bytes:number|null;error:string|null}[];truncated:boolean;error:string|null};
type Snapshot={current:string|null;fingerprint:string;can_apply:boolean;reason:string|null;observation:string;backup_name:string|null;candidate_hash:string;operation_state?:string;can_restore?:boolean;recovery_original?:{name:string;status:string;error:string|null}|null;staging?:StagingInventory|null};
const stagingKinds:Record<string,string>={file:"File — contents not checked",directory:"Folder — not searched",link:"Link — not followed",other:"Other entry — not checked",unavailable:"Entry could not be inspected"};
const pending=(row:ReviewSummary)=>row.state==="prepared"||row.state==="review_required";
const interrupted=(row:ReviewSummary)=>row.state==="applying"||row.state==="backed_up"||row.state==="restoring";
export function XmpRecovery({workspace,refresh,onChanged}:{workspace:string;refresh:number;onChanged?:()=>void}){
  const [rows,setRows]=useState<ReviewSummary[]>([]),[selected,setSelected]=useState<ReviewSummary|null>(null),[record,setRecord]=useState<Review|null>(null),[snapshot,setSnapshot]=useState<Snapshot|null>(null);
  const [busy,setBusy]=useState(false),[error,setError]=useState(""),[loading,setLoading]=useState(true),[revision,setRevision]=useState(0),[history,setHistory]=useState(false),[confirmed,setConfirmed]=useState(false),[message,setMessage]=useState("");
  const generation=useRef(0);
  const heading=useRef<HTMLHeadingElement>(null),focusPage=useRef<string|null>(null);
  const scope=JSON.stringify([workspace,refresh,revision,history]);
  const [paging,setPaging]=useState<{scope:string;cursors:(number|null)[]}>({scope:"",cursors:[null]});
  const cursors=paging.scope===scope?paging.cursors:[null],before=cursors[cursors.length-1];
  const [nextBefore,setNextBefore]=useState<number|null>(null);
  useEffect(()=>{setMessage("");setHistory(false)},[workspace]);
  useEffect(()=>{
    const token=++generation.current;setRows([]);setNextBefore(null);setSelected(null);setRecord(null);setSnapshot(null);setConfirmed(false);setError("");setLoading(true);setBusy(false);
    invoke<ReviewPage>("xmp_review_page",{workspace,history,before}).then(value=>{if(generation.current===token){setRows(value.items);setNextBefore(value.next_before)}}).catch(cause=>{if(generation.current===token)setError(String(cause))}).finally(()=>{if(generation.current===token)setLoading(false)});
    return()=>{generation.current++};
  },[workspace,refresh,revision,history,before]);
  useEffect(()=>{if(!loading&&focusPage.current){if(focusPage.current===scope)heading.current?.focus();focusPage.current=null}},[loading,scope]);
  function turnPage(older:boolean){
    if(busy||loading||(older?nextBefore===null:cursors.length===1))return;
    focusPage.current=scope;setPaging({scope,cursors:older?[...cursors,nextBefore]:cursors.slice(0,-1)});
  }
  async function inspect(row:ReviewSummary){
    const token=++generation.current;setSelected(row);setRecord(null);setSnapshot(null);setConfirmed(false);setError("");setMessage("");setBusy(true);
    try{
      const saved=await invoke<Review>("xmp_review_record",{workspace,id:row.id});
      if(token!==generation.current)return;
      if(saved.id!==row.id||saved.path!==row.path||saved.candidate_hash!==row.candidate_hash)throw new Error("The proposal changed. Refresh saved changes before reviewing again.");
      setRecord(saved);setSelected(saved);
      const value=await invoke<Snapshot>("xmp_review_snapshot",{workspace,id:row.id});
      if(token!==generation.current)return;
      if(value.candidate_hash!==saved.candidate_hash)throw new Error("The proposal changed. Refresh saved changes before reviewing again.");
      setSnapshot(value);setSelected({...saved,state:value.operation_state||saved.state});
    }
    catch(cause){if(token===generation.current)setError(String(cause))}
    finally{if(token===generation.current)setBusy(false)}
  }
  async function decide(row:ReviewSummary,decision:"kept_external"|"cancelled"){
    if(busy||(decision==="kept_external"&&!snapshot?.fingerprint))return;
    const token=generation.current;setBusy(true);setError("");
    try{await invoke("xmp_review_decide",{workspace,id:row.id,decision,fingerprint:decision==="cancelled"?row.candidate_hash:snapshot!.fingerprint});if(token===generation.current){setMessage(decision==="cancelled"?"Proposal cancelled. All files and saved snapshots were kept.":"Current sidecar kept. Saved snapshots were retained.");setRevision(n=>n+1);if(decision==="kept_external")onChanged?.()}}
    catch(cause){if(token===generation.current){setError(String(cause));setSnapshot(null);setConfirmed(false)}}
    finally{if(token===generation.current)setBusy(false)}
  }
  async function apply(){
    if(!selected||!snapshot?.can_apply||!confirmed||busy)return;
    const token=generation.current;setBusy(true);setError("");
    try{const value=await invoke<{state:string;backup_name:string|null}>("xmp_review_apply",{workspace,id:selected.id,fingerprint:snapshot.fingerprint,candidateHash:selected.candidate_hash});if(token===generation.current){setMessage(value.backup_name?"Reviewed changes applied. The previous sidecar was kept as a recovery original.":"Reviewed sidecar created. No existing file was overwritten.");setRevision(n=>n+1);onChanged?.()}}
    catch(cause){if(token===generation.current){setError(String(cause));setSnapshot(null);setConfirmed(false)}}
    finally{if(token===generation.current)setBusy(false)}
  }
  async function restore(){
    if(!selected||!snapshot?.can_restore||!confirmed||busy)return;
    const token=generation.current;setBusy(true);setError("");
    try{await invoke("xmp_review_restore",{workspace,id:selected.id,fingerprint:snapshot.fingerprint,candidateHash:selected.candidate_hash});if(token===generation.current){setMessage("Saved original restored. Both versions remain in saved history.");setRevision(n=>n+1);onChanged?.()}}
    catch(cause){if(token===generation.current){setError(String(cause));setSnapshot(null);setConfirmed(false)}}
    finally{if(token===generation.current)setBusy(false)}
  }
  return <section className="xmp-recovery" aria-labelledby="xmp-recovery-title">
    <h2 id="xmp-recovery-title" ref={heading} tabIndex={-1}>XMP changes to review</h2>
    <p>Compare the current sidecar with the proposed version before applying. Open-Photo keeps the previous version and refuses to overwrite a competing file.</p>
    <label><input type="checkbox" checked={history} disabled={busy} onChange={event=>setHistory(event.target.checked)}/> Show completed saves and decisions</label>
    {loading&&<p role="status">Loading saved changes…</p>}
    {message&&<p role="status">{message}</p>}
    {error&&<p role="alert" className="error">{error}</p>}
    {!loading&&!rows.length&&!error&&<p>{cursors.length>1?"No changes remain on this page. Refresh saved changes to see the latest records.":history?"No saved XMP operations.":"No pending XMP changes."}</p>}
    <button className="secondary-button" disabled={busy||loading} onClick={()=>setRevision(n=>n+1)}>Refresh saved changes</button>
    <ul>{rows.map(row=><li key={row.id}><span className="break-anywhere">{row.path}</span><small> · {pending(row)?"Pending review":interrupted(row)?"Interrupted save — inspect recovery files":row.state==="published"||row.state==="applied"?"Saved":row.state==="restored"?"Original restored":row.state==="cancelled"?"Cancelled":"Current version kept"}</small> <button className="secondary-button" disabled={busy} onClick={()=>void inspect(row)}>{pending(row)?"Review external changes":"Inspect saved versions"}</button>{pending(row)&&selected?.id!==row.id&&<button className="secondary-button" disabled={busy} onClick={()=>void decide(row,"cancelled")}>Cancel proposed change</button>}</li>)}</ul>
    {(cursors.length>1||nextBefore!==null)&&<nav className="feedback-actions xmp-review-pages" aria-label="XMP review pages">
      <button className="secondary-button" disabled={busy||loading||cursors.length===1} onClick={()=>turnPage(false)}>Newer changes</button>
      {!loading&&<p role="status">Page {cursors.length} · {rows.length} {history?"saved operations":"pending changes"}</p>}
      <button className="secondary-button" disabled={busy||loading||nextBefore===null} onClick={()=>turnPage(true)}>Older changes</button>
    </nav>}
    {selected&&<div className="xmp-review-detail" aria-busy={busy}>
      <h3 className="break-anywhere">Review {selected.path.split(/[\\/]/).at(-1)}</h3>
      {busy&&!record&&<p role="status">Reading saved versions…</p>}
      {busy&&record&&!snapshot&&<p role="status">Verifying the original media and current sidecar…</p>}
      {record?.integrity_error&&<p role="alert" className="error">{record.integrity_error}</p>}
      {record&&!snapshot&&!busy&&<p className="xmp-review-unavailable">Saved versions are available for inspection. The current file could not be verified, so Apply, restoration and Keep current version remain unavailable.</p>}
      {!snapshot&&record?.backup_name&&<p className="break-anywhere">Recorded recovery original name: {record.backup_name}</p>}
      {snapshot&&<>
        <p>{snapshot.observation==="missing"?"The sidecar is currently missing. Nothing will be restored automatically.":snapshot.observation==="matches_saved"?"The current sidecar matches the saved candidate.":snapshot.observation==="changed"?"The current sidecar differs from the saved candidate.":snapshot.observation==="inaccessible"?"The current file is unavailable. Stored versions are still retained.":"Current file inspected."}</p>
        {snapshot.reason&&<p className="results-note">{snapshot.reason}</p>}
        {snapshot.backup_name&&<p className="break-anywhere">Recorded recovery original name: {snapshot.backup_name}</p>}
        {snapshot.recovery_original&&<p>{selected.state==="restored"?"The original was restored; its recorded recovery name is retained for reference.":snapshot.recovery_original.status==="verified"?"The saved original’s identity and contents match the recovery record.":`Saved original unavailable: ${snapshot.recovery_original.error||"It could not be verified."}`}</p>}
      </>}
      {record&&<>
        <div className="xmp-review-versions"><section><h4>Current sidecar</h4><pre>{snapshot?snapshot.observation==="inaccessible"?"Unable to read the current sidecar.":snapshot.current??"No sidecar currently exists.":busy?"Verification in progress…":"The current version could not be verified."}</pre></section><section><h4>{record.integrity_error?"Unverified stored proposal":"Proposed version"}</h4><pre>{record.candidate}</pre></section></div>
        <details open={snapshot?.can_restore||undefined}><summary>Original snapshot</summary><pre>{record.original??"This operation began without an existing sidecar."}</pre></details>
      </>}
      {snapshot?.staging&&<details className="xmp-stage-files">
        <summary>Possible staging files</summary>
        <p>These names may belong to this save. Their contents and ownership have not been checked. Nothing listed here is applied or deleted automatically.</p>
        {snapshot.staging.truncated&&<p>Only part of this folder was checked. More matching files may exist.</p>}
        {snapshot.staging.error&&<p className="break-anywhere">{snapshot.staging.error}</p>}
        {!snapshot.staging.entries.length&&!snapshot.staging.truncated&&!snapshot.staging.error&&<p>No matching filenames were seen in this check.</p>}
        <ul>{snapshot.staging.entries.map((entry,index)=><li key={`${entry.name}-${index}`}>
          <span className="break-anywhere">{entry.name}</span><span> · {stagingKinds[entry.kind]||"Unverified entry"}{typeof entry.bytes==="number"?` · ${entry.bytes.toLocaleString()} bytes`:""}</span>
          {entry.error&&<p className="break-anywhere">{entry.error}</p>}
        </li>)}</ul>
      </details>}
      {snapshot&&<>
        {pending(selected)&&<>
          {snapshot.can_apply&&!record?.integrity_error&&<><label><input type="checkbox" checked={confirmed} disabled={busy} onChange={event=>setConfirmed(event.target.checked)}/> I reviewed the current and proposed versions.</label><div><button className="primary-button" disabled={busy||!confirmed} onClick={()=>void apply()}>Apply reviewed changes</button></div></>}
          <div className="feedback-actions"><button className="secondary-button" disabled={busy||!snapshot.fingerprint} onClick={()=>void decide(selected,"kept_external")}>Keep external version</button></div>
        </>}
        {interrupted(selected)&&<><p>The save was interrupted. Keeping the current version records your choice and retains every recovery snapshot and file.</p><button className="secondary-button" disabled={busy||!snapshot.fingerprint||snapshot.current===null} onClick={()=>void decide(selected,"kept_external")}>Keep current version</button></>}
        {snapshot.can_restore&&interrupted(selected)&&<><p>The sidecar path is empty. You can explicitly restore the verified original; any file created there in the meantime will be kept instead.</p><label><input type="checkbox" checked={confirmed} disabled={busy} onChange={event=>setConfirmed(event.target.checked)}/> I reviewed the saved original and want to restore it.</label><div><button className="primary-button" disabled={busy||!confirmed} onClick={()=>void restore()}>Restore saved original</button></div></>}
      </>}
      {!snapshot&&!busy&&<button className="secondary-button" onClick={()=>void inspect(selected)}>{record?"Read current version again":"Read saved versions again"}</button>}
      {pending(selected)&&<button className="secondary-button" disabled={busy} onClick={()=>void decide(selected,"cancelled")}>Cancel proposed change</button>}
      <button className="secondary-button" disabled={busy} onClick={()=>{generation.current++;setSelected(null);setRecord(null);setSnapshot(null);setConfirmed(false)}}>Close review</button>
    </div>}
  </section>
}
