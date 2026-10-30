import { greet } from "./sample";

export class Widget {
  render() {
    return greet("x");
  }
}

export function makeWidget() {
  return new Widget();
}

const helper = () => 1;
