import {invoke} from "@tauri-apps/api/core";

const desktop=typeof window!=="undefined"&&"__TAURI_INTERNALS__" in window;
let pending:Promise<unknown>=Promise.resolve();
let settingsClosing=false;
let hydrated=false;
let values=new Map<string,string>();
const browserFallback=new Set<string>();
const bootstrapKeys=new Set(["openphoto.theme","openphoto.accent"]);
const failures=new Map<string,string>();
export function setSettingsClosing(value:boolean){settingsClosing=value;}

/** SQLite owns desktop persistence; this snapshot owns the live renderer view. */
export function readSetting(key:string):string|null{
  if(hydrated||browserFallback.has(key))return values.get(key)??null;
  try{return localStorage.getItem(key);}
  catch{return values.get(key)??null;}
}

function legacySettings(){
  const legacy:Record<string,string>={};
  try{
    for(let index=0;index<localStorage.length;index++){
      const key=localStorage.key(index);
      if(key?.startsWith("openphoto.")){
        const value=localStorage.getItem(key);
        if(value!==null)legacy[key]=value;
      }
    }
  }catch{/* Browser storage may be unavailable; native data remains authoritative. */}
  return legacy;
}

export async function hydrateSettings(){
  if(!desktop||hydrated)return;
  const saved=await invoke<Record<string,string>>("load_user_data",{legacy:legacySettings()});
  // Replace the live view only after native loading succeeds. No quota-sized mirror.
  values=new Map(Object.entries(saved));
  hydrated=true;
  for(const key of bootstrapKeys){
    const value=values.get(key);
    try{if(value!==undefined&&value.length<=256)localStorage.setItem(key,value);}catch{/* Optional bootstrap cache. */}
  }
}

export function saveSetting(key:string,value:string){
  if(settingsClosing)throw Error("The app is closing; keep it open before changing settings.");
  values.set(key,value);
  if(!desktop||!hydrated||(bootstrapKeys.has(key)&&value.length<=256)){
    try{localStorage.setItem(key,value);browserFallback.delete(key);if(!desktop)failures.delete(key);}
    catch(error){
      browserFallback.add(key);
      // A failed optional browser mirror is not a failed native save.
      if(!desktop){failures.set(key,String(error));report(error);}
    }
  }
  if(!desktop)return;
  // Preserve write order so a slow older write cannot replace newer metadata.
  pending=pending.catch(()=>{}).then(()=>invoke("save_user_data",{key,value})).then(()=>{failures.delete(key)});
  void pending.catch(error=>{failures.set(key,String(error));report(error)});
}

function report(error:unknown){
  window.dispatchEvent(new CustomEvent("openphoto-storage-error",{detail:String(error)}));
}

export async function retrySettings(){
  // Retry exact live values, including edits that never fit in browser storage.
  const snapshot=hydrated?new Map(values):new Map([...Object.entries(legacySettings()),...values]);
  for(const [key,value] of snapshot)saveSetting(key,value);
  await flushSettings();
}

/** Wait for saved ownership/folder settings before native project commands read them. */
export async function flushSettings(keys?:string[]){
  await pending.catch(()=>{});
  const errors=[...failures].filter(([key])=>!keys||keys.includes(key)).map(([,error])=>error);
  if(errors.length)throw Error(errors.join("; "));
}
