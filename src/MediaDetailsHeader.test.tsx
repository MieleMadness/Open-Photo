import {cleanup,fireEvent,render,screen} from "@testing-library/react";
import {afterEach,expect,it,vi} from "vitest";
import {MediaDetailsHeader} from "./MediaDetailsHeader";
afterEach(cleanup);
it("keeps the short viewer actions accessible and preserves navigation and busy states",()=>{
  const props={video:true,editing:false,fullscreen:false,canPrevious:false,canNext:true,onEdit:vi.fn(),onFullscreen:vi.fn(),onPrevious:vi.fn(),onNext:vi.fn(),onClose:vi.fn()};
  const {rerender}=render(<MediaDetailsHeader {...props}/>);
  fireEvent.click(screen.getByRole("button",{name:"Edit"}));expect(props.onEdit).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button",{name:"Fullscreen"}));expect(props.onFullscreen).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button",{name:"Previous item"}));expect(props.onPrevious).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button",{name:"Next item"}));expect(props.onNext).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("button",{name:"Close details"}));expect(props.onClose).toHaveBeenCalledOnce();
  rerender(<MediaDetailsHeader {...props} editing fullscreen/>);
  expect((screen.getByRole("button",{name:"Opening…"}) as HTMLButtonElement).disabled).toBe(true);
  expect(screen.getByRole("button",{name:"Restore"}).getAttribute("aria-pressed")).toBe("true");
  rerender(<MediaDetailsHeader {...props} video={false}/>);expect(screen.queryByRole("button",{name:"Edit"})).toBeNull();
});
