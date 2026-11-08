import {useEffect,useRef,useState} from "react";

/** A reply owns one request in one library scope, including its errors and busy state. */
export function useScopedTask<T>(scope:string){
  const owner=useRef({scope,request:0,mounted:true});
  if(owner.current.scope!==scope){owner.current.scope=scope;owner.current.request++;}
  const [state,setState]=useState<{scope:string;value:T|null;busy:boolean;error:string}>({scope,value:null,busy:false,error:""});
  useEffect(()=>{owner.current.mounted=true;return()=>{owner.current.mounted=false;owner.current.request++;};},[]);
  function reset(){owner.current.request++;setState({scope,value:null,busy:false,error:""});}
  async function run(work:()=>Promise<T>){
    if(!owner.current.mounted||owner.current.scope!==scope)return;
    const request=++owner.current.request;
    const current=()=>owner.current.mounted&&owner.current.scope===scope&&owner.current.request===request;
    setState({scope,value:null,busy:true,error:""});
    try{const value=await work();if(current())setState({scope,value,busy:false,error:""});}
    catch(error){if(current())setState({scope,value:null,busy:false,error:String(error)});}
  }
  return {value:state.scope===scope?state.value:null,busy:state.scope===scope&&state.busy,error:state.scope===scope?state.error:"",run,reset};
}
