import {describe,expect,it} from "vitest";
import {isMajorStableRelease} from "./appUpdater";

describe("major stable updater policy",()=>{
  it("accepts only N.0.0 public releases",()=>{
    expect(isMajorStableRelease("1.0.0")).toBe(true);
    expect(isMajorStableRelease("v2.0.0")).toBe(true);
    expect(isMajorStableRelease("10.0.0")).toBe(true);
  });
  it("rejects previews, zero-major, minor, and patch releases",()=>{
    for(const version of ["0.56.0","1.1.0","1.0.1","2.3.4","1.0.0-beta.1","v0.0.0"]) expect(isMajorStableRelease(version)).toBe(false);
  });
});
