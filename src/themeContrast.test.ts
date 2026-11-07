import css from "./tokens.css?raw";
import {expect,it} from "vitest";

function colors(block:string){return Object.fromEntries([...block.matchAll(/(--[\w-]+):\s*(#[0-9a-f]+)/gi)].map(match=>[match[1],match[2]]));}
const light=colors(css.match(/:root\s*\{([^}]+)\}/)![1]);
const dark={...light,...colors(css.match(/:root\[data-theme="dark"\]\s*\{([^}]+)\}/)![1])};
function luminance(hex:string){
  let value=hex.slice(1);if(value.length===3)value=[...value].map(c=>c+c).join("");
  const channels=[0,2,4].map(i=>parseInt(value.slice(i,i+2),16)/255).map(v=>v<=0.04045?v/12.92:((v+0.055)/1.055)**2.4);
  return channels[0]*0.2126+channels[1]*0.7152+channels[2]*0.0722;
}
const variants=(["light","dark"] as const).flatMap(theme=>["purple","blue","teal","green","rose"].map(accent=>({theme,accent})));
it.each(variants)("keeps enabled semantic text readable in $theme / $accent",({theme,accent})=>{
  const palette=colors(css.match(new RegExp(':root\\[data-accent="'+accent+'"\\]\\s*\\{([^}]+)\\}'))?.[1]||"");
  const darkPalette=theme==="dark"?colors(css.match(new RegExp(':root\\[data-theme="dark"\\]\\[data-accent="'+accent+'"\\]\\s*\\{([^}]+)\\}'))?.[1]||""):{};
  const tokens={...(theme==="light"?light:dark),...palette,...darkPalette};
  const pairs=[["--text","--bg"],["--text","--surface"],["--muted","--surface"],["--muted","--surface-muted"],["--muted","--primary-soft"],["--accent-text","--primary-soft"],["--on-primary","--primary"],["--on-primary","--primary-hover"],["--success-text","--green-soft"],["--danger","--danger-soft"]];
  for(const [foreground,background] of pairs){const values=[luminance(tokens[foreground]),luminance(tokens[background])].sort((a,b)=>a-b);expect((values[1]+0.05)/(values[0]+0.05),`${foreground} on ${background}`).toBeGreaterThanOrEqual(4.5);}
});
