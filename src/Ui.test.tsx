import {useRef,useState} from "react";
import {cleanup,fireEvent,render,screen,waitFor} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {Modal,NoticeSurface,NoticeTarget,TaskForm} from "./Ui";
afterEach(cleanup);

it("moves live notice controls into the active host without losing the owner's state",()=>{
  function Owner(){const [count,setCount]=useState(0);return <NoticeSurface><button onClick={()=>setCount(n=>n+1)}>Job state {count}</button></NoticeSurface>;}
  function Harness(){const target=useRef<HTMLDivElement>(null),[host,setHost]=useState<HTMLElement|null>(null);return <NoticeTarget.Provider value={host}><Owner/><button onClick={()=>setHost(target.current)}>Move into dialog</button><div data-testid="notice-host" ref={target}/></NoticeTarget.Provider>;}
  render(<Harness/>);fireEvent.click(screen.getByRole("button",{name:"Job state 0"}));fireEvent.click(screen.getByRole("button",{name:"Move into dialog"}));expect(screen.getByTestId("notice-host").contains(screen.getByRole("button",{name:"Job state 1"}))).toBe(true);
});

it("keeps dialog focus within visible controls and restores the invoking control",()=>{
  function Harness(){const [open,setOpen]=useState(false);return <><button onClick={()=>setOpen(true)}>Open details</button>{open&&<Modal label="Details" onClose={()=>setOpen(false)}><button>First action</button><details><summary>Advanced</summary><button>Hidden action</button></details><button onClick={()=>setOpen(false)}>Close</button></Modal>}</>;}
  render(<Harness/>);const trigger=screen.getByRole("button",{name:"Open details"});trigger.focus();fireEvent.click(trigger);
  const first=screen.getByRole("button",{name:"First action"}),last=screen.getByRole("button",{name:"Close"});expect(document.activeElement).toBe(first);
  fireEvent.keyDown(first,{key:"Tab",shiftKey:true});expect(document.activeElement).toBe(last);
  fireEvent.keyDown(last,{key:"Tab"});expect(document.activeElement).toBe(first);
  fireEvent.keyDown(first,{key:"Escape"});expect(screen.queryByRole("dialog")).toBeNull();expect(document.activeElement).toBe(trigger);
});

it("retains an invalid form draft, identifies its field, and submits cleaned values",async()=>{
  const save=vi.fn(),close=vi.fn();render(<TaskForm title="Create workspace" fields={[{name:"name",label:"Workspace name"}]} onSubmit={save} onClose={close}/>);
  const field=screen.getByLabelText("Workspace name");fireEvent.change(field,{target:{value:"   "}});fireEvent.submit(field.closest("form")!);
  expect(save).not.toHaveBeenCalled();expect((field as HTMLInputElement).value).toBe("   ");expect(field.getAttribute("aria-invalid")).toBe("true");expect(document.activeElement).toBe(field);
  fireEvent.change(field,{target:{value:"  Holiday videos  "}});fireEvent.submit(field.closest("form")!);expect(save).toHaveBeenCalledWith({name:"Holiday videos"});await waitFor(()=>expect(close).toHaveBeenCalledOnce());
});

it("keeps form input available when an asynchronous save fails",async()=>{
  const close=vi.fn();render(<TaskForm title="Name person" fields={[{name:"name",label:"Person name",value:"Family"}]} onSubmit={async()=>{throw Error("Local data is unavailable");}} onClose={close}/>);
  fireEvent.click(screen.getByRole("button",{name:"Save"}));await screen.findByText(/Local data is unavailable/);
  expect((screen.getByLabelText("Person name") as HTMLInputElement).value).toBe("Family");expect(close).not.toHaveBeenCalled();expect((screen.getByRole("button",{name:"Save"}) as HTMLButtonElement).disabled).toBe(false);
});
