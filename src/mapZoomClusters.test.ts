import {expect,it} from "vitest";
import {clusterMapLocations} from "./mapZoomClusters";
const photos=[{path:"a.jpg",lat:27.9500,lon:-82.4600},{path:"b.jpg",lat:27.9504,lon:-82.4604},{path:"c.jpg",lat:40.7,lon:-74}];
it("combines nearby photos when zoomed out and separates them when zoomed in",()=>{
  const wide=clusterMapLocations(photos,5);
  const close=clusterMapLocations(photos,19);
  expect(wide.some(group=>group.photos.length===2)).toBe(true);
  expect(close).toHaveLength(3);
});
it("keeps valid polar media visible within Leaflet projection bounds",()=>{
  const north={path:"north.jpg",lat:90,lon:30};
  const south={path:"south.jpg",lat:-90,lon:-30};
  const groups=clusterMapLocations([north,south],8);
  expect(groups.flatMap(group=>group.photos)).toEqual([north,south]);
  expect(groups.map(group=>group.lat)).toEqual([85,-85]);
});
it("retains all valid media and excludes malformed coordinates",()=>{
  const grouped=clusterMapLocations([...photos,{path:"broken",lat:NaN,lon:1}],8);
  expect(grouped.flatMap(group=>group.photos.map(photo=>photo.path)).sort()).toEqual(["a.jpg","b.jpg","c.jpg"]);
});

it("centers grouped markers independently of input file order",()=>{
  const pair=[{path:"one.jpg",lat:27.9500,lon:-82.4600},{path:"two.jpg",lat:27.9504,lon:-82.4604}];
  const forward=clusterMapLocations(pair,5);
  const reverse=clusterMapLocations([...pair].reverse(),5);
  expect(forward).toHaveLength(1);
  expect(reverse).toHaveLength(1);
  expect(forward[0].lat).toBeCloseTo(27.9502,6);
  expect(forward[0].lon).toBeCloseTo(-82.4602,6);
  expect(reverse[0].lat).toBeCloseTo(forward[0].lat,6);
  expect(reverse[0].lon).toBeCloseTo(forward[0].lon,6);
});
