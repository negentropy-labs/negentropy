import { Repo } from "./repo";

export function resolveName(id: string): string {
  const repo = new Repo();
  return repo.findById(id);
}
