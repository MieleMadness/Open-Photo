import {expect,it} from "vitest";
import {libraryFolderChoices,matchesLibraryFolder,folderLabel} from "./libraryFolderSelection";
it("selects direct children by root and directory, never substrings",()=>{
  const selection={root:"C:\\Photos",directory:"Trips"};
  expect(matchesLibraryFolder({path:"c:/photos/TRIPS/one.jpg"},"C:/Photos",selection)).toBe(true);
  for(const path of ["C:/Photos/Trips-old/one.jpg","C:/Photos/Trips/nested/one.jpg","C:/Photos/Trips.jpg","D:/Photos/Trips/one.jpg"])expect(matchesLibraryFolder({path},"C:/Photos",selection),path).toBe(false);
  expect(matchesLibraryFolder({path:"C:/Photos/one.jpg"},"C:/Photos",{root:"C:/Photos",directory:""})).toBe(true);
  expect(matchesLibraryFolder({path:"C:/Photos/Trips/one.jpg"},"C:/Photos",{root:"C:/Photos",directory:""})).toBe(false);
});
it("retains source-root identity and normalizes verbatim, UNC and Unicode paths",()=>{
  const photos=[{libraryRoot:"C:/ФОТО",path:"C:/ФОТО/Trips/one.jpg"},{libraryRoot:"D:/ФОТО",path:"D:/ФОТО/Trips/one.jpg"}];
  const choices=libraryFolderChoices(photos,"");expect(choices).toHaveLength(2);
  expect(folderLabel(choices[0],true)).toContain("C:/ФОТО");
  expect(matchesLibraryFolder(photos[1],"",choices[0])).toBe(false);
  const unc={root:"\\\\server\\share",directory:"Trips"};
  expect(matchesLibraryFolder({path:"\\\\?\\UNC\\SERVER\\share\\Trips\\one.jpg",libraryRoot:"\\\\server\\share"},"",unc)).toBe(true);
  expect(libraryFolderChoices([{path:"\\\\?\\C:\\ФОТО\\Trips\\one.jpg",libraryRoot:"C:/ФОТО"}],"")[0].directory).toBe("Trips");
});
