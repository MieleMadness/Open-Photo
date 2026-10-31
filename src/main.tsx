import React from "react";
import { createRoot } from "react-dom/client";
import {hydrateSettings} from "./userData";
import App from "./App";
import {ErrorNotice} from "./Ui";
import "./styles.css";
import "./editor.css";

const host=document.getElementById("root")!;
const root=createRoot(host);
async function start(){
  try{
    await hydrateSettings();
    root.render(<React.StrictMode><App /></React.StrictMode>);
  }catch(error){
    root.render(<main className="startup-recovery"><h1>Open-Photo</h1><ErrorNotice title="Your saved library could not be opened" error={error}><button className="primary-button" onClick={()=>void start()}>Retry opening</button></ErrorNotice><p>Your saved data has not been replaced.</p></main>);
  }
}
void start();
