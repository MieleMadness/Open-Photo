import {invoke,convertFileSrc} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
export type VaultItem={id:string;name:string;mime:string;bytes:number};
export type VaultStatus={available:boolean;configured:boolean;retry_after_seconds:number;reason:string|null};
export type VaultGrant={token:string;epoch:number;items:VaultItem[]};
export type VaultLockNotice={through_epoch:number};
export type VaultReply={lock?:VaultLockNotice;status?:VaultStatus;grant?:VaultGrant;items?:VaultItem[]};
export type VaultAction={type:"status"|"lock"}|{type:"setup";credential:string;confirmation:string}|{type:"unlock";credential:string}|{type:"list"|"touch"|"check"|"cancel"|"revoke";token:string}|{type:"import";token:string;path:string}|{type:"change_pin";token:string;current:string;credential:string;confirmation:string}|{type:"backup";token:string;destination:string}|{type:"restore";path:string;credential:string};
function items(value:unknown):value is VaultItem[]{return Array.isArray(value)&&value.length<=500&&value.every(o=>o&&/^[a-f0-9]{32}$/.test(o.id)&&typeof o.name==="string"&&o.name.length>0&&o.name.length<=512&&typeof o.mime==="string"&&/^(image\/(jpeg|png|webp|gif|bmp)|video\/(mp4|webm))$/.test(o.mime)&&Number.isSafeInteger(o.bytes)&&o.bytes>0&&o.bytes<=512*1024*1024);}
export function lockEpoch(value:unknown):number{const notice=value as VaultLockNotice|null;if(!notice||!Number.isSafeInteger(notice.through_epoch)||notice.through_epoch<0)throw Error("The vault lock notification could not be verified.");return notice.through_epoch;}
export async function vaultRequest(action:VaultAction,current?:()=>boolean):Promise<VaultReply>{
 if(["setup","unlock","restore"].includes(action.type))await flushSettings(["openphoto.activeWorkspace","openphoto.workspaces"]);
 if(current&&!current())throw Error("Vault request cancelled before authentication.");
 const value=await invoke<VaultReply>("vault_command",{action});
 if(!value||typeof value!=="object"||(value.items!==undefined&&!items(value.items))||(value.grant!==undefined&&(!/^[a-f0-9]{64}$/.test(value.grant.token)||!items(value.grant.items)||!Number.isSafeInteger(value.grant.epoch)||value.grant.epoch<1))||(value.status!==undefined&&(typeof value.status.available!=="boolean"||typeof value.status.configured!=="boolean"||!Number.isSafeInteger(value.status.retry_after_seconds)||value.status.retry_after_seconds<0||(value.status.reason!==null&&typeof value.status.reason!=="string"))))throw Error("The vault response could not be verified. Lock and reopen the vault.");
 if(value.lock!==undefined)lockEpoch(value.lock);
 if((action.type==="lock"||action.type==="revoke")&&value.lock===undefined)throw Error("The native vault lock was not confirmed.");
 return value;
}
export function vaultMedia(token:string,id:string){if(!/^[a-f0-9]{64}$/.test(token)||!/^[a-f0-9]{32}$/.test(id))throw Error("Invalid vault media identity");return `${convertFileSrc(token,"vault")}/${id}`;}
export function credentialError(value:string){if(new TextEncoder().encode(value).length>256)return "This PIN or passphrase is too long. Use a shorter one.";return value.includes("\0")||(/^\d+$/.test(value)?value.length<6:[...value].length<12)?"Use at least six digits, or a passphrase of at least twelve characters.":"";}
