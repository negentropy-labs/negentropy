import { shared } from "./state";

export function setBusy(): void {
  shared.status = "busy";
  shared.version += 1;
}
