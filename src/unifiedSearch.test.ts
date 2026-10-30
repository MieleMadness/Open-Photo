import {expect,it} from "vitest";
import {mediaIdentity,parseSearch,rankSearch,type SearchEvidence,type SearchMedia} from "./unifiedSearch";
const media:SearchMedia[]=[{path:"C:/A/burger.jpg",name:"burger.jpg",extension:"jpg",libraryRoot:"C:/A",modified_unix:10},{path:"C:/A/IMG_2.jpg",name:"IMG_2.jpg",extension:"jpg",libraryRoot:"C:/A",modified_unix:10},{path:"C:/B/game.mp4",name:"game.mp4",extension:"mp4",libraryRoot:"C:/B",modified_unix:20}];
const evidence=(index:number,kind:SearchEvidence["kind"],text="",score=1):SearchEvidence=>({root:media[index].libraryRoot!,path:media[index].path,modified_unix:media[index].modified_unix,kind,text,score,timestamp_seconds:null});
const rank=(query:string,e:SearchEvidence[]=[],tags:Record<string,string[]>={},ratings:Record<string,number>={})=>rankSearch(media,"",parseSearch(query),tags,ratings,e);
it("ranks explicit filename and tags ahead of weaker meaning matches while preserving all reasons",()=>{
 const hits=rank("burger",[evidence(1,"semantic","",.55),evidence(0,"semantic","",.4)],{[media[0].path]:["food/burger"]});expect(hits.map(h=>h.path)).toEqual([media[0].path,media[1].path]);expect(hits[0].reasons.map(r=>r.kind)).toContain("filename");expect(hits[0].reasons.map(r=>r.kind)).toContain("tag");expect(hits[0].reasons.map(r=>r.kind)).toContain("semantic");
});
it("combines terms across filename, tags, OCR and named people rather than requiring one field to contain everything",()=>{
 const e=[{...evidence(0,"ocr","Receipt from lunch"),matched_terms:["receipt"]},evidence(0,"person","Alex Lee")];const hits=rank('burger receipt person:"Alex Lee"',e);expect(hits).toHaveLength(1);expect(hits[0].reasons.map(r=>r.kind)).toEqual(expect.arrayContaining(["filename","ocr","person"]));
});
it("applies ratings/favorites, types, hierarchical tags and paths as AND constraints",()=>{
 const tags={[media[2].path]:["family/holidays"]},ratings={[media[2].path]:5,[media[0].path]:4};expect(rank('type:video tag:family rating:>=4 is:favorite path:"C:/B"',[],tags,ratings).map(h=>h.path)).toEqual([media[2].path]);expect(rank('rating:4',[],tags,ratings).map(h=>h.path)).toEqual([media[0].path]);expect(rank('type:photo is:favorite',[],tags,ratings)).toEqual([]);
});
it("rejects stale, foreign-root, removed, invalid and low-confidence indexed evidence",()=>{
 const valid=evidence(1,"ocr","burger");expect(rank("burger",[{...valid,modified_unix:9},{...valid,root:"foreign"},{...valid,path:"deleted.jpg"},evidence(2,"semantic","",NaN),evidence(2,"semantic","",.19)]).map(h=>h.path)).toEqual([media[0].path]);expect(mediaIdentity("a","b/c")).not.toBe(mediaIdentity("a/b","c"));
});
it("deduplicates evidence without inflating scores and retains valid video timestamps for Phase F",()=>{
 const e={...evidence(2,"video_semantic","",.6),timestamp_seconds:12.5};const once=rank("burger",[e]),many=rank("burger",Array(100).fill(e));expect(many).toEqual(once);expect(once.find(h=>h.media_type==="video")?.matched_timestamps).toEqual([12.5]);expect(rank("burger",[{...e,timestamp_seconds:-1}]).some(h=>h.media_type==="video")).toBe(false);expect(rank("burger",[{...evidence(1,"video_semantic","",.7),timestamp_seconds:1}]).some(h=>h.path===media[1].path)).toBe(false);
});
it("supports quoted Unicode names and literal paths while surfacing invalid field values",()=>{
 expect(parseSearch('person:"Zoë Lee" family dinner').person).toBe("zoë lee");expect(parseSearch('C:\\Media\\trip').text).toBe('c:\\media\\trip');expect(parseSearch("rating:6").error).not.toBe("");expect(rank("rating:6")).toEqual([]);expect(parseSearch("x".repeat(501)).error).not.toBe("");
});
