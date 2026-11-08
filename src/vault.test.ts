import {afterEach,beforeEach,expect,it,vi} from "vitest";
import {invoke} from "@tauri-apps/api/core";
import {flushSettings} from "./userData";
import {vaultRequest,vaultMedia,credentialError} from "./vault";
vi.mock("@tauri-apps/api/core",()=>({invoke:vi.fn(),convertFileSrc:(p:string,scheme:string)=>`${scheme}://localhost/${encodeURIComponent(p)}`}));
vi.mock("./userData",()=>({flushSettings:vi.fn(async()=>{})}));
beforeEach(()=>{vi.clearAllMocks();vi.mocked(invoke).mockResolvedValue({});});
it("flushes workspace identity before authentication without storing the credential",async()=>{await vaultRequest({type:"unlock",credential:"public test passphrase"});expect(flushSettings).toHaveBeenCalledWith(["openphoto.activeWorkspace","openphoto.workspaces"]);expect(invoke).toHaveBeenCalledWith("vault_command",{action:{type:"unlock",credential:"public test passphrase"}});expect(localStorage.length).toBe(0);});
it("rejects unsupported replies and malformed media identities",async()=>{vi.mocked(invoke).mockResolvedValue({grant:{token:"bad",items:[]}});await expect(vaultRequest({type:"unlock",credential:"public test passphrase"})).rejects.toThrow(/could not be verified/);expect(()=>vaultMedia("bad","../escape")).toThrow();expect(vaultMedia("a".repeat(64),"b".repeat(32))).toBe(`vault://localhost/${"a".repeat(64)}/${"b".repeat(32)}`);});
it("accepts pasted long PINs/passphrases and rejects short or oversized credentials",()=>{for(const valid of ["123456","12345678901234567890","a pasted passphrase"])expect(credentialError(valid)).toBe("");for(const invalid of ["12345","short","a".repeat(257),"hello\0password"])expect(credentialError(invalid)).not.toBe("");});
afterEach(()=>localStorage.clear());

it("cancels authentication waiting for workspace persistence before native invocation",async()=>{let finish!:()=>void;vi.mocked(flushSettings).mockImplementationOnce(()=>new Promise<void>(resolve=>{finish=resolve;}));let current=true;const pending=vaultRequest({type:"unlock",credential:"public test passphrase"},()=>current);current=false;finish();await expect(pending).rejects.toThrow(/cancelled/);expect(invoke).not.toHaveBeenCalled();});
it("requires a verified native lock acknowledgement",async()=>{await expect(vaultRequest({type:"lock"})).rejects.toThrow(/not confirmed/);vi.mocked(invoke).mockResolvedValue({lock:{through_epoch:3}});await expect(vaultRequest({type:"lock"})).resolves.toEqual({lock:{through_epoch:3}});vi.mocked(invoke).mockResolvedValue({lock:{through_epoch:-1}});await expect(vaultRequest({type:"lock"})).rejects.toThrow(/could not be verified/);});

it("matches the native UTF-8 passphrase budget without truncating Unicode",()=>{for(const value of ["é".repeat(128),"😀".repeat(64)])expect(credentialError(value)).toBe("");for(const value of ["é".repeat(129),"😀".repeat(65)])expect(credentialError(value)).toMatch(/too long/);});
