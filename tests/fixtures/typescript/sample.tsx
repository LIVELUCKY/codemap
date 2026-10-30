import React from "react";
import { greet } from "./sample";

export function Hello(props: { name: string }) {
  return <h1>{greet(props.name)}</h1>;
}

export const Counter = () => <button>0</button>;
