import {expect,it} from "vitest";
import {ThumbnailCache} from "./thumbnailCache";
it("evicts least recently used previews under byte and item caps",()=>{
  const cache=new ThumbnailCache(60,3);
  cache.set("a","a".repeat(10));cache.set("b","b".repeat(10));
  expect(cache.get("a")).toBe("a".repeat(10));
  cache.set("c","c".repeat(10));cache.set("d","d".repeat(10));
  expect(cache.get("b")).toBeUndefined();
  expect(cache.get("a")).toBe("a".repeat(10));
  expect(cache.size).toBe(3);
  expect(cache.estimatedBytes).toBeLessThanOrEqual(60);
});
it("replaces stale cache entries and rejects oversized data",()=>{
  const cache=new ThumbnailCache(20,2);
  cache.set("source-v1","old");
  cache.set("source-v2","new");
  cache.set("source-v2","fresh");
  expect(cache.get("source-v2")).toBe("fresh");
  cache.set("oversize","x".repeat(100));
  expect(cache.get("oversize")).toBeUndefined();
  expect(cache.estimatedBytes).toBeLessThanOrEqual(20);
});
