import { shared } from "./state";

export function setDone(): void {
  shared.status = "done";
}
