import {expect,it} from "vitest";
import {validMapLocations} from "./mapLocations";
it("accepts signed coordinates in both hemispheres including world boundaries",()=>{
  const points=[{path:"south-west.jpg",lat:-27.5,lon:-82.5},{path:"boundary.jpg",lat:90,lon:180},{path:"origin.jpg",lat:0,lon:0}];
  expect(validMapLocations(points)).toEqual(points);
});
it("rejects out-of-range, nonfinite and empty-path map points",()=>{
  expect(validMapLocations([{path:"bad.jpg",lat:91,lon:0},{path:"bad2.jpg",lat:0,lon:-181},{path:"nan.jpg",lat:NaN,lon:0},{path:"inf.jpg",lat:0,lon:Infinity},{path:"",lat:12,lon:13},{path:"good.jpg",lat:-90,lon:-180}])).toEqual([{path:"good.jpg",lat:-90,lon:-180}]);
});
