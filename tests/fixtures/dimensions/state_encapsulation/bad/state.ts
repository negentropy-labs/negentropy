export const shared = {
  status: "idle",
  version: 0,
};

export function setup(): void {
  let pending = true;
  let retries = 3;
  let tag = "x";
  if (pending) {
    retries -= 1;
    tag = `${tag}:${retries}`;
  }
}
