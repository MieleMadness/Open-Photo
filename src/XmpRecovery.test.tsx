import {render,screen,fireEvent,waitFor,cleanup} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {XmpRecovery} from "./XmpRecovery";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn()}));
afterEach(()=>{cleanup();vi.resetAllMocks()});
const row={id:7,path:"C:\\Photos\\sample.jpg",state:"review_required",original:"<xmp>original</xmp>",candidate:"<xmp>proposed</xmp>",original_hash:"a",candidate_hash:"b"};
const view={current:"<xmp>external</xmp>",fingerprint:"fresh",can_apply:true,reason:null,observation:"changed",backup_name:null,candidate_hash:"b"};
function mockBackend(handler:(cmd:string,args:unknown)=>unknown,record?:(id:number)=>unknown){
 let loadedRows:(typeof row&{integrity_error?:string})[]=[];
 vi.mocked(invoke).mockImplementation(async(cmd,args)=>{
  if(cmd==="xmp_review_record")return record?record((args as {id:number}).id):loadedRows.find(item=>item.id===(args as {id:number}).id);
  const result=await handler(cmd,args);
  if(cmd==="xmp_review_page"){
   const page=Array.isArray(result)?{items:result,next_before:null}:result as {items:typeof loadedRows;next_before:number|null};
   loadedRows=page.items;
   return {...page,items:page.items.map(({id,path,state,candidate_hash})=>({id,path,state,candidate_hash}))};
  }
  return result;
 });
}
it("requires review confirmation before Apply and sends the exact candidate binding",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:cmd==="xmp_review_snapshot"?view:{state:"applied",backup_name:"original.bak"});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByText("<xmp>external</xmp>");
 expect((screen.getByText("Apply reviewed changes") as HTMLButtonElement).disabled).toBe(true);
 fireEvent.click(screen.getByLabelText("I reviewed the current and proposed versions."));fireEvent.click(screen.getByText("Apply reviewed changes"));
 await waitFor(()=>expect(invoke).toHaveBeenCalledWith("xmp_review_apply",{workspace:"w",id:7,fingerprint:"fresh",candidateHash:"b"}));expect(await screen.findByText(/Reviewed changes applied/)).toBeTruthy();
});
it("closing a review changes no files or journal decisions",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:view);render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByText("<xmp>external</xmp>");fireEvent.click(screen.getByText("Close review"));expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="xmp_review_decide"||cmd==="xmp_review_apply")).toBe(false);
});
it("allows journal-only cancellation when current media cannot be read",async()=>{
 mockBackend(async(cmd)=>{if(cmd==="xmp_review_page")return [row];if(cmd==="xmp_review_snapshot")throw Error("Media moved");return null});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByRole("alert");expect(screen.getByText(row.candidate)).toBeTruthy();fireEvent.click(screen.getByText("Original snapshot"));expect(screen.getByText(row.original)).toBeTruthy();fireEvent.click(screen.getByText("Cancel proposed change"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("xmp_review_decide",{workspace:"w",id:7,decision:"cancelled",fingerprint:"b"}));
});
it("invalidates review and consent after a stale Apply refusal",async()=>{
 mockBackend(async(cmd)=>{if(cmd==="xmp_review_page")return [row];if(cmd==="xmp_review_snapshot")return view;throw new Error("The sidecar changed; review again")});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByText("<xmp>external</xmp>");fireEvent.click(screen.getByLabelText("I reviewed the current and proposed versions."));fireEvent.click(screen.getByText("Apply reviewed changes"));expect(await screen.findByRole("alert")).toBeTruthy();expect(screen.queryByText("Apply reviewed changes")).toBeNull();expect(screen.getByText("Read current version again")).toBeTruthy();
});
it("flags a corrupt row while keeping healthy reviews usable",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[{...row,id:8,integrity_error:"Damaged snapshot retained"},row]:view);render(<XmpRecovery workspace="w" refresh={0}/>);await screen.findAllByText("Review external changes");expect(screen.getAllByText("Review external changes")).toHaveLength(2);fireEvent.click(screen.getAllByText("Review external changes")[0]);await screen.findByText("Damaged snapshot retained");expect(screen.queryByText("Apply reviewed changes")).toBeNull();fireEvent.click(screen.getByText("Close review"));fireEvent.click(screen.getAllByText("Review external changes")[1]);expect(await screen.findByText("Apply reviewed changes")).toBeTruthy();
});
it("offers completed history and reports missing files without applying automatically",async()=>{
 mockBackend(async(cmd,args)=>cmd==="xmp_review_page"?(args as {history:boolean}).history?[{...row,state:"published"}]:[]:{...view,current:null,can_apply:false,observation:"missing",reason:"History only"});
 render(<XmpRecovery workspace="w" refresh={0}/>);await screen.findByText("No pending XMP changes.");fireEvent.click(screen.getByLabelText("Show completed saves and decisions"));fireEvent.click(await screen.findByText("Inspect saved versions"));await screen.findByText("The sidecar is currently missing. Nothing will be restored automatically.");expect(screen.queryByText("Apply reviewed changes")).toBeNull();expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="xmp_review_apply")).toBe(false);
});
it("refuses a candidate changed between the list and the snapshot",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:{...view,candidate_hash:"changed"});render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));expect(await screen.findByRole("alert")).toBeTruthy();expect(screen.queryByText("Apply reviewed changes")).toBeNull();
});
it("ignores a snapshot returned after the workspace changes",async()=>{
 let resolve!:(value:unknown)=>void;mockBackend(async(cmd,args)=>cmd==="xmp_review_page"?(args as {workspace:string}).workspace==="w"?[row]:[]:new Promise(r=>{resolve=r}));const rendered=render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await waitFor(()=>expect(resolve).toBeTypeOf("function"));rendered.rerender(<XmpRecovery workspace="other" refresh={0}/>);await screen.findByText("No pending XMP changes.");resolve(view);await waitFor(()=>expect(screen.queryByText("<xmp>external</xmp>")).toBeNull());
});
it("can acknowledge an interrupted save after reading the current version without applying again",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[{...row,state:"applying"}]:cmd==="xmp_review_snapshot"?{...view,can_apply:false,reason:"Interrupted save"}:null);
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Inspect saved versions"));await screen.findByText("Keep current version");expect(screen.queryByText("Apply reviewed changes")).toBeNull();fireEvent.click(screen.getByText("Keep current version"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("xmp_review_decide",{workspace:"w",id:7,decision:"kept_external",fingerprint:"fresh"}));
});
it("requires explicit confirmation before restoring a verified original to an empty path",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[{...row,state:"applying"}]:cmd==="xmp_review_snapshot"?{...view,current:null,can_apply:false,can_restore:true,operation_state:"restoring",recovery_original:{name:"original.bak",status:"verified",error:null},observation:"missing"}:{state:"restored"});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Inspect saved versions"));await screen.findByText("Restore saved original");expect((screen.getByText("Restore saved original") as HTMLButtonElement).disabled).toBe(true);expect((screen.getByText("Keep current version") as HTMLButtonElement).disabled).toBe(true);fireEvent.click(screen.getByLabelText("I reviewed the saved original and want to restore it."));fireEvent.click(screen.getByText("Restore saved original"));await waitFor(()=>expect(invoke).toHaveBeenCalledWith("xmp_review_restore",{workspace:"w",id:7,fingerprint:"fresh",candidateHash:"b"}));expect(await screen.findByText("Saved original restored. Both versions remain in saved history.")).toBeTruthy();
});
it("does not offer restoration for an unverified backup",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[{...row,state:"applying"}]:{...view,current:null,can_apply:false,can_restore:false,observation:"missing",recovery_original:{name:"original.bak",status:"unavailable",error:"Recovery identity changed"}});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Inspect saved versions"));await screen.findByText(/Saved original unavailable/);expect(screen.queryByText("Restore saved original")).toBeNull();
});
it("invalidates restoration approval when a competing file makes the token stale",async()=>{
 mockBackend(async(cmd)=>{if(cmd==="xmp_review_page")return [{...row,state:"applying"}];if(cmd==="xmp_review_snapshot")return {...view,current:null,can_apply:false,can_restore:true,observation:"missing"};throw Error("Competing sidecar exists; original retained")});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Inspect saved versions"));await screen.findByText("Restore saved original");fireEvent.click(screen.getByLabelText("I reviewed the saved original and want to restore it."));fireEvent.click(screen.getByText("Restore saved original"));await screen.findByRole("alert");expect(screen.queryByText("Restore saved original")).toBeNull();expect(screen.getByText("Read current version again")).toBeTruthy();
});
it("refreshes metadata after success but ignores an old workspace's late Apply result",async()=>{
 const changed=vi.fn();let complete!:(value:unknown)=>void;
 mockBackend(async(cmd,args)=>{if(cmd==="xmp_review_page")return (args as {workspace:string}).workspace==="w"?[row]:[];if(cmd==="xmp_review_snapshot")return view;if(cmd==="xmp_review_apply")return new Promise(resolve=>{complete=resolve});return null});
 const rendered=render(<XmpRecovery workspace="w" refresh={0} onChanged={changed}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByText("Apply reviewed changes");fireEvent.click(screen.getByLabelText("I reviewed the current and proposed versions."));fireEvent.click(screen.getByText("Apply reviewed changes"));rendered.rerender(<XmpRecovery workspace="other" refresh={0} onChanged={changed}/>);await screen.findByText("No pending XMP changes.");complete({state:"applied",backup_name:"old.bak"});await waitFor(()=>expect(screen.queryByText(/Reviewed changes applied/)).toBeNull());expect(changed).not.toHaveBeenCalled();
});
it("notifies the tag browser after a successful restoration",async()=>{
 const changed=vi.fn();mockBackend(async(cmd)=>cmd==="xmp_review_page"?[{...row,state:"applying"}]:cmd==="xmp_review_snapshot"?{...view,current:null,can_apply:false,can_restore:true,operation_state:"applying",observation:"missing"}:{state:"restored"});
 render(<XmpRecovery workspace="w" refresh={0} onChanged={changed}/>);fireEvent.click(await screen.findByText("Inspect saved versions"));await screen.findByText("Restore saved original");fireEvent.click(screen.getByLabelText("I reviewed the saved original and want to restore it."));fireEvent.click(screen.getByText("Restore saved original"));await screen.findByText("Saved original restored. Both versions remain in saved history.");expect(changed).toHaveBeenCalledTimes(1);
});

it("loads stored documents only when a summary is opened",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:view);
 render(<XmpRecovery workspace="w" refresh={0}/>);await screen.findByText(row.path);
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="xmp_review_record"||cmd==="xmp_review_snapshot")).toBe(false);
 expect(screen.queryByText(row.candidate)).toBeNull();fireEvent.click(screen.getByText("Review external changes"));
 expect(await screen.findByText(row.candidate)).toBeTruthy();expect(invoke).toHaveBeenCalledWith("xmp_review_record",{workspace:"w",id:7});
});
it("pages older and newer summaries without retaining old approval",async()=>{
 const older={...row,id:6,path:"C:\\Photos\\older.jpg"};
 mockBackend(async(cmd,args)=>cmd==="xmp_review_page"?(args as {before:number|null}).before===null?{items:[row],next_before:7}:{items:[older],next_before:null}:view);
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByText(row.candidate);fireEvent.click(screen.getByLabelText("I reviewed the current and proposed versions."));
 fireEvent.click(screen.getByText("Older changes"));await screen.findByText(older.path);expect(screen.queryByText(row.path)).toBeNull();expect(screen.queryByText("Apply reviewed changes")).toBeNull();expect((screen.getByText("Older changes") as HTMLButtonElement).disabled).toBe(true);
 expect(invoke).toHaveBeenCalledWith("xmp_review_page",{workspace:"w",history:false,before:7});expect(document.activeElement).toBe(screen.getByText("XMP changes to review"));
 fireEvent.click(screen.getByText("Newer changes"));await screen.findByText(row.path);expect(screen.queryByText(older.path)).toBeNull();fireEvent.click(screen.getByText("Review external changes"));await screen.findByText(row.candidate);expect((screen.getByText("Apply reviewed changes") as HTMLButtonElement).disabled).toBe(true);
});
it("resets paging for history, workspace switches and refresh",async()=>{
 mockBackend(async(cmd,args)=>cmd==="xmp_review_page"?{items:[{...row,path:`${(args as {workspace:string}).workspace}-${(args as {before:number|null}).before??"first"}`}],next_before:7}:view);
 const rendered=render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Older changes"));await screen.findByText("w-7");
 fireEvent.click(screen.getByLabelText("Show completed saves and decisions"));await screen.findByText("w-first");expect(invoke).toHaveBeenLastCalledWith("xmp_review_page",{workspace:"w",history:true,before:null});
 fireEvent.click(screen.getByText("Older changes"));await screen.findByText("w-7");fireEvent.click(screen.getByText("Refresh saved changes"));await screen.findByText("w-first");
 fireEvent.click(screen.getByText("Older changes"));await screen.findByText("w-7");rendered.rerender(<XmpRecovery workspace="other" refresh={0}/>);await screen.findByText("other-first");expect(invoke).toHaveBeenLastCalledWith("xmp_review_page",{workspace:"other",history:false,before:null});
});
it("ignores an older page returned after changing workspaces",async()=>{
 let complete!:(value:unknown)=>void;
 mockBackend(async(cmd,args)=>{
  if(cmd!=="xmp_review_page")return view;
  const {workspace,before}=args as {workspace:string;before:number|null};
  if(workspace==="other")return [{...row,path:"Other workspace"}];
  return before===null?{items:[row],next_before:7}:new Promise(resolve=>{complete=resolve});
 });
 const rendered=render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Older changes"));await waitFor(()=>expect(complete).toBeTypeOf("function"));
 rendered.rerender(<XmpRecovery workspace="other" refresh={0}/>);await screen.findByText("Other workspace");complete({items:[{...row,path:"Stale old page"}],next_before:null});
 await waitFor(()=>expect(screen.queryByText("Stale old page")).toBeNull());expect(screen.getByText("Other workspace")).toBeTruthy();
});
it("keeps a way back when an older page fails",async()=>{
 mockBackend(async(cmd,args)=>{
  if(cmd!=="xmp_review_page")return view;
  if((args as {before:number|null}).before!==null)throw Error("Could not read this page");
  return {items:[row],next_before:7};
 });
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Older changes"));await screen.findByRole("alert");fireEvent.click(screen.getByText("Newer changes"));await screen.findByText(row.path);expect(screen.queryByRole("alert")).toBeNull();
});
it("refuses details changed since their summary was listed",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:view,()=>({...row,candidate_hash:"different"}));
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByRole("alert");expect(screen.queryByText("Apply reviewed changes")).toBeNull();
});

it("shows stored documents during a slow current check without enabling writes",async()=>{
 let complete!:(value:unknown)=>void;
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:new Promise(resolve=>{complete=resolve}));
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByText(row.candidate);
 expect(screen.getByText("Verifying the original media and current sidecar…")).toBeTruthy();expect(screen.queryByText("Apply reviewed changes")).toBeNull();expect(screen.queryByLabelText("I reviewed the current and proposed versions.")).toBeNull();
 complete(view);expect((await screen.findByText("Apply reviewed changes") as HTMLButtonElement).disabled).toBe(true);
});
it("keeps stored documents after verification fails and requires fresh review after retry",async()=>{
 let online=false;
 mockBackend(async(cmd)=>{if(cmd==="xmp_review_page")return [row];if(!online)throw Error("Folder access was removed");return view});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByRole("alert");expect(screen.getByText(row.candidate)).toBeTruthy();fireEvent.click(screen.getByText("Original snapshot"));expect(screen.getByText(row.original)).toBeTruthy();
 expect(screen.queryByText("Apply reviewed changes")).toBeNull();expect(screen.queryByText("Restore saved original")).toBeNull();expect(screen.queryByText("Keep external version")).toBeNull();
 online=true;fireEvent.click(screen.getByText("Read current version again"));expect((await screen.findByText("Apply reviewed changes") as HTMLButtonElement).disabled).toBe(true);expect(screen.queryByRole("alert")).toBeNull();
 expect(vi.mocked(invoke).mock.calls.filter(([cmd])=>cmd==="xmp_review_record")).toHaveLength(2);
});
it("does not verify current media when its saved record cannot be loaded",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:view,()=>{throw Error("Saved record unavailable")});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByRole("alert");expect(screen.getByText("Read saved versions again")).toBeTruthy();expect(screen.queryByText(row.candidate)).toBeNull();
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="xmp_review_snapshot")).toBe(false);
});
it("discards a late saved record without starting a current check in the old workspace",async()=>{
 let complete!:(value:unknown)=>void;
 mockBackend(async(cmd,args)=>cmd==="xmp_review_page"?(args as {workspace:string}).workspace==="w"?[row]:[]:view,()=>new Promise(resolve=>{complete=resolve}));
 const rendered=render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await waitFor(()=>expect(complete).toBeTypeOf("function"));
 rendered.rerender(<XmpRecovery workspace="other" refresh={0}/>);await screen.findByText("No pending XMP changes.");complete(row);await waitFor(()=>expect(screen.queryByText(row.candidate)).toBeNull());
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="xmp_review_snapshot")).toBe(false);
});
it("retains interrupted documents and the recorded backup name during an offline check",async()=>{
 const interrupted={...row,state:"applying",backup_name:".openphoto-xmp-original-7-123.bak"};
 mockBackend(async(cmd)=>{if(cmd==="xmp_review_page")return [interrupted];throw Error("Source unavailable")});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Inspect saved versions"));await screen.findByRole("alert");expect(screen.getByText(row.candidate)).toBeTruthy();expect(screen.getByText(`Recorded recovery original name: ${interrupted.backup_name}`)).toBeTruthy();
 expect(screen.queryByText("Restore saved original")).toBeNull();expect(screen.queryByText("Keep current version")).toBeNull();expect(screen.queryByText("Cancel proposed change")).toBeNull();
});
it.each([{id:8},{path:"C:\\Photos\\different.jpg"}])("refuses mismatched record identity before any current read: %o",async changed=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:view,()=>({...row,...changed}));
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByRole("alert");expect(screen.queryByText(row.candidate)).toBeNull();expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="xmp_review_snapshot")).toBe(false);
});

it("shows staging observations as unverified metadata with no file actions",async()=>{
 const staging={entries:[{name:".openphoto-xmp-123-7-456.tmp",kind:"file",bytes:1234,error:null},{name:".openphoto-xmp-124-7-457.tmp",kind:"link",bytes:null,error:null},{name:".openphoto-xmp-125-7-458.tmp",kind:"directory",bytes:null,error:null}],truncated:false,error:null};
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:{...view,staging});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));fireEvent.click(await screen.findByText("Possible staging files"));
 expect(screen.getByText(staging.entries[0].name)).toBeTruthy();expect(screen.getByText(`· File — contents not checked · ${(1234).toLocaleString()} bytes`)).toBeTruthy();expect(screen.getByText("· Link — not followed")).toBeTruthy();expect(screen.getByText("· Folder — not searched")).toBeTruthy();
 expect(screen.getByText("Possible staging files").closest("details")!.querySelectorAll("button")).toHaveLength(0);expect((screen.getByText("Apply reviewed changes") as HTMLButtonElement).disabled).toBe(true);
 expect(vi.mocked(invoke).mock.calls.some(([cmd])=>cmd==="xmp_review_apply"||cmd==="xmp_review_restore"||cmd==="xmp_review_decide")).toBe(false);
});
it("does not describe partial or failed staging inspection as an empty folder",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:{...view,staging:{entries:[],truncated:true,error:"Could not finish listing files"}});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));fireEvent.click(await screen.findByText("Possible staging files"));
 expect(screen.getByText("Only part of this folder was checked. More matching files may exist.")).toBeTruthy();expect(screen.getByText("Could not finish listing files")).toBeTruthy();expect(screen.queryByText("No matching filenames were seen in this check.")).toBeNull();
});
it("reports a complete empty staging observation without claiming recovery success",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:{...view,staging:{entries:[],truncated:false,error:null}});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));fireEvent.click(await screen.findByText("Possible staging files"));expect(screen.getByText("No matching filenames were seen in this check.")).toBeTruthy();expect(screen.queryByText("Only part of this folder was checked. More matching files may exist.")).toBeNull();
});
it("does not imply staging files were checked when no observation is available",async()=>{
 mockBackend(async(cmd)=>cmd==="xmp_review_page"?[row]:{...view,staging:null});
 render(<XmpRecovery workspace="w" refresh={0}/>);fireEvent.click(await screen.findByText("Review external changes"));await screen.findByText("Apply reviewed changes");expect(screen.queryByText("Possible staging files")).toBeNull();
});
