import {readSetting,saveSetting,flushSettings} from "./userData";
import type {Workspace} from "./libraryWorkspace";
export type Collection={id:string;workspaceId:string;name:string;paths:string[]};
export type SmartRule={id:string;workspaceId:string;name:string;minRating:number;type:"all"|"photo"|"video";minMB:number};
type Settings={workspaces:Workspace[];collections:Collection[];smartRules:SmartRule[];ratings:Record<string,number>;peopleNames:Record<string,string>;duplicateReview:Record<string,string>};
type Key=keyof Settings;
const record=(value:unknown):value is Record<string,unknown>=>!!value&&typeof value==="object"&&!Array.isArray(value);
const strings=(value:unknown):value is string[]=>Array.isArray(value)&&value.every(item=>typeof item==="string");
const identity=(value:Record<string,unknown>)=>typeof value.id==="string"&&!!value.id&&typeof value.name==="string";
const owned=(value:Record<string,unknown>)=>identity(value)&&typeof value.workspaceId==="string";
const unique=(value:unknown[],guard:(item:Record<string,unknown>)=>boolean)=>value.every(item=>record(item)&&guard(item))&&new Set(value.map(item=>(item as {id:string}).id)).size===value.length;
const schemas:Record<Key,(value:unknown)=>boolean>={
  workspaces:value=>Array.isArray(value)&&unique(value,item=>identity(item)&&typeof item.root==="string"&&(item.roots===undefined||strings(item.roots))),
  collections:value=>Array.isArray(value)&&unique(value,item=>owned(item)&&strings(item.paths)),
  smartRules:value=>Array.isArray(value)&&unique(value,item=>owned(item)&&["all","photo","video"].includes(String(item.type))&&Number.isInteger(item.minRating)&&Number(item.minRating)>=0&&Number(item.minRating)<=5&&typeof item.minMB==="number"&&Number.isFinite(item.minMB)&&item.minMB>=0),
  ratings:value=>record(value)&&Object.values(value).every(rating=>Number.isInteger(rating)&&Number(rating)>=0&&Number(rating)<=5),
  peopleNames:value=>record(value)&&Object.values(value).every(name=>typeof name==="string"),
  duplicateReview:value=>record(value)&&Object.values(value).every(path=>typeof path==="string"),
};
export class SettingsRepairError extends Error {
  constructor(public key:Key,public original:string,reason:string){super(`Saved ${key} need repair: ${reason}. The saved value has been preserved.`);}
}
export function validateStructuredSetting<K extends Key>(key:K,raw:string):Settings[K]{
  let value:unknown;
  try{value=JSON.parse(raw);}catch{throw new SettingsRepairError(key,raw,"invalid JSON");}
  // Bare values are the supported legacy format. A v1 envelope can be repaired/migrated
  // into that format; future envelopes stay blocked rather than being rewritten.
  if(record(value)&&"schemaVersion" in value){
    if(value.schemaVersion!==1)throw new SettingsRepairError(key,raw,"unsupported schema version");
    value=value.value;
  }
  if(!schemas[key](value))throw new SettingsRepairError(key,raw,"unexpected data shape");
  return value as Settings[K];
}
export function readStructuredSetting<K extends Key>(key:K):Settings[K]{
  const raw=readSetting(`openphoto.${key}`);
  return validateStructuredSetting(key,raw??(["workspaces","collections","smartRules"].includes(key)?"[]":"{}"));
}
/** Only an explicit validated repair writes; failed rendering never saves empty defaults. */
export async function repairStructuredSetting(key:Key,raw:string){
  const value=validateStructuredSetting(key,raw);
  saveSetting(`openphoto.${key}`,JSON.stringify(value));
  await flushSettings([`openphoto.${key}`]);
}
