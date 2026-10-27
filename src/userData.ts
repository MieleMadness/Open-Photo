import {invoke} from "@tauri-apps/api/core";

const desktop=typeof window!=="undefined"&&"__TAURI_INTERNALS__" in window;
let pending:Promise<unknown>=Promise.resolve();

export async function hydrateSettings(){
  if(!desktop)return;
  const legacy:Record<string,string>={};
  for(let index=0;index<localStorage.length;index++){
    const key=localStorage.key(index);
    if(key?.startsWith("openphoto."))legacy[key]=localStorage.getItem(key)!;
  }
  const saved=await invoke<Record<string,string>>("load_user_data",{legacy});
  for(const [key,value] of Object.entries(saved))localStorage.setItem(key,value);
}

export function saveSetting(key:string,value:string){
  try{localStorage.setItem(key,value)}catch(error){report(error)}
  if(!desktop)return;
  // Preserve write order so a slow older write cannot replace newer metadata.
  pending=pending.catch(()=>{}).then(()=>invoke("save_user_data",{key,value}));
  void pending.catch(report);
}

function report(error:unknown){
  window.dispatchEvent(new CustomEvent("openphoto-storage-error",{detail:String(error)}));
}

export async function retrySettings(){
  for(let index=0;index<localStorage.length;index++){
    const key=localStorage.key(index);
    if(key?.startsWith("openphoto."))saveSetting(key,localStorage.getItem(key)!);
  }
  await pending;
}
