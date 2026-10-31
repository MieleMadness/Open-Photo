import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {AiDashboard} from "./AiDashboard";
import type {IndexReport} from "./useAiIndex";
afterEach(cleanup);
const counts={indexed:2,pending:3,failed:1,partial:1,unsupported:4,excluded:0};
const report:IndexReport={counts,folders:[{path:"C:/Pictures",excluded:false,inherited:false,counts}],failures:[],failure_total:1,running:true,error:null,revision:1,generation:1};
it("shows durable file counters and folder exclusions, with inspectable/retryable failures",async()=>{
 const exclude=vi.fn(async()=>{}),retry=vi.fn(async()=>{}),failures=vi.fn(async()=>({items:[{root:"C:/Pictures",path:"C:/Pictures/broken.jpg",service:"ocr",error:"Decode failed",attempted:100}],total:1}));
 render(<AiDashboard report={report} error="" enabled onExclude={exclude} onRetry={retry} onFailures={failures}/>);
 expect(screen.getByText("2 indexed")).toBeTruthy();fireEvent.click(screen.getByRole("button",{name:"Don't index"}));await waitFor(()=>expect(exclude).toHaveBeenCalledWith("C:/Pictures",true));
 fireEvent.click(screen.getByRole("button",{name:/View failures/}));await screen.findByText("C:/Pictures/broken.jpg");expect(failures).toHaveBeenCalledWith("",0);
 fireEvent.click(screen.getByRole("button",{name:"Retry file"}));await waitFor(()=>expect(retry).toHaveBeenCalledWith("C:/Pictures/broken.jpg"));
});
it("keeps off-state retry disabled and explains inherited exclusions",()=>{
 render(<AiDashboard report={{...report,folders:[{...report.folders[0],excluded:true,inherited:true}]}} error="" enabled={false} onExclude={vi.fn()} onRetry={vi.fn()} onFailures={vi.fn()}/>);
 expect((screen.getByRole("button",{name:"Excluded by parent"}) as HTMLButtonElement).disabled).toBe(true);
 expect((screen.getByRole("button",{name:"Retry failed / resume"}) as HTMLButtonElement).disabled).toBe(true);
});
