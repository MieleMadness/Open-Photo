import type {ReactNode} from "react";
/** Green section labels are an explicit product preference, shared by every main page/state. */
export function PageHeading({eyebrow,title,description,actions,className=""}:{eyebrow:string;title:ReactNode;description:ReactNode;actions?:ReactNode;className?:string}){
  return <div className={`page-heading ${className}`.trim()}><div><span className="eyebrow">{eyebrow}</span><h1>{title}</h1><p>{description}</p></div>{actions}</div>;
}
