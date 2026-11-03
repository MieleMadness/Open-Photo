export type RootedPath = {path:string;libraryRoot?:string};

/** Request limits bound individual calls; they must never truncate a library. */
export async function readMetadataBatches<T>(
  photos: RootedPath[], root: string, batchSize: number,
  read: (root:string, paths:string[])=>Promise<T[]>,
  active: ()=>boolean = ()=>true,
): Promise<{values:T[];errors:string[]}> {
  const grouped=new Map<string,string[]>();
  for(const photo of photos){
    const parent=photo.libraryRoot||root;
    if(!parent)continue;
    const paths=grouped.get(parent)||[];
    paths.push(photo.path);grouped.set(parent,paths);
  }
  const values:T[]=[];const errors:string[]=[];
  for(const [parent,paths] of grouped){
    for(let offset=0;offset<paths.length;offset+=batchSize){
      if(!active())return {values,errors};
      try{values.push(...await read(parent,paths.slice(offset,offset+batchSize)))}
      catch(error){errors.push(`${parent}: ${String(error)}`)}
    }
  }
  return {values,errors};
}
