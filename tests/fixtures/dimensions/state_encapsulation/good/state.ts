export const state = {
  status: "idle",
  version: 1,
};

export function snapshot(): string {
  const text = `${state.status}:${state.version}`;
  return text;
}
