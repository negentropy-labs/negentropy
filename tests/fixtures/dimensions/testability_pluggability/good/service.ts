import { Repo } from "./repo";

export function resolveName(repo: Repo, id: string): string {
  const raw = repo.findById(id);
  const upper = raw.toUpperCase();
  return upper;
}
