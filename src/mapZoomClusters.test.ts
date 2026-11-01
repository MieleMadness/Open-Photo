import {expect,it} from "vitest";
import {clusterMapLocations} from "./mapZoomClusters";
const photos=[{path:"a.jpg",lat:27.9500,lon:-82.4600},{path:"b.jpg",lat:27.9504,lon:-82.4604},{path:"c.jpg",lat:40.7,lon:-74}];
it("combines nearby photos when zoomed out and separates them when zoomed in",()=>{
  const wide=clusterMapLocations(photos,5);
  const close=clusterMapLocations(photos,19);
  expect(wide.some(group=>group.photos.length===2)).toBe(true);
  expect(close).toHaveLength(3);
});
it("retains all valid media and excludes malformed coordinates",()=>{
  const grouped=clusterMapLocations([...photos,{path:"broken",lat:NaN,lon:1}],8);
  expect(grouped.flatMap(group=>group.photos.map(photo=>photo.path)).sort()).toEqual(["a.jpg","b.jpg","c.jpg"]);
});
