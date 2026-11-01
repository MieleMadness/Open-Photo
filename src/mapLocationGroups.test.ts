import {expect,it} from "vitest";
import {groupNearbyLocations} from "./mapLocationGroups";
it("groups overlapping GPS locations but preserves every selectable photo",()=>{
  const groups=groupNearbyLocations([{path:"one.jpg",lat:27.950001,lon:-82.460001},{path:"two.jpg",lat:27.950002,lon:-82.460002},{path:"far.jpg",lat:29,lon:-80}]);
  expect(groups).toHaveLength(2);
  expect(groups[0].photos.map(item=>item.path)).toEqual(["one.jpg","two.jpg"]);
  expect(groups[1].photos[0].path).toBe("far.jpg");
});
it("excludes invalid GPS points from group markers",()=>{
  expect(groupNearbyLocations([{path:"invalid",lat:NaN,lon:2},{path:"valid",lat:0,lon:0}])).toHaveLength(1);
});
