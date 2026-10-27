import React from "react";
import { createRoot } from "react-dom/client";
import {hydrateSettings} from "./userData";
import App from "./App";
import "./styles.css";

const host=document.getElementById("root")!;
async function start(){
  try{
    await hydrateSettings();
    createRoot(host).render(<React.StrictMode><App /></React.StrictMode>);
  }catch(error){
    const message=document.createElement("p");
    message.textContent=`Open-Photo could not load your saved data. Your data has not been replaced. ${String(error)}`;
    const retry=document.createElement("button");retry.textContent="Retry opening";retry.onclick=()=>{host.replaceChildren();void start()};
    host.replaceChildren(message,retry);
  }
}
void start();
