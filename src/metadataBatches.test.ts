import {describe,it,expect,vi} from "vitest";
import {readMetadataBatches} from "./metadataBatches";

describe("metadata batches",()=>{
  it("includes items beyond 5,000 and keeps root boundaries",async()=>{
    const photos=Array.from({length:5001},(_,i)=>({path:`a/${i}`,libraryRoot:"a"}));
    photos.push({path:"b/0",libraryRoot:"b"});
    const read=vi.fn(async(root:string,paths:string[])=>paths.map(path=>`${root}:${path}`));
    const result=await readMetadataBatches(photos,"",1000,read);
    expect(result.values).toContain("a:a/5000");expect(result.values).toContain("b:b/0");
    expect(result.values).toHaveLength(5002);
    expect(read.mock.calls.every(([,paths])=>paths.length<=1000)).toBe(true);
  });
  it("reports failed batches while retaining successes",async()=>{
    const result=await readMetadataBatches([{path:"1"},{path:"2"},{path:"3"}],"root",1,async(_,paths)=>{
      if(paths[0]==="2")throw Error("Disconnected");return paths;
    });
    expect(result.values).toEqual(["1","3"]);expect(result.errors[0]).toContain("Disconnected");
  });
  it("stops requesting work after cancellation",async()=>{
    let active=true;const read=vi.fn(async()=>{active=false;return ["one"]});
    await readMetadataBatches([{path:"1"},{path:"2"}],"root",1,read,()=>active);
    expect(read).toHaveBeenCalledTimes(1);
  });
});
