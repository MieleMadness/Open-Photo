import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it} from "vitest";
import {EditorProjectMenu} from "./EditorProjectMenu";
afterEach(cleanup);
it("dismisses the project menu with Escape and outside interaction",()=>{
  render(<><EditorProjectMenu><button>Save copy</button></EditorProjectMenu><button>Outside</button></>);
  const summary=screen.getByText("Project"),details=summary.closest("details")!;
  details.open=true;screen.getByRole("button",{name:"Save copy"}).focus();fireEvent.keyDown(details,{key:"Escape"});
  expect(details.open).toBe(false);expect(document.activeElement).toBe(summary);
  details.open=true;fireEvent.pointerDown(screen.getByRole("button",{name:"Outside"}));expect(details.open).toBe(false);
});
