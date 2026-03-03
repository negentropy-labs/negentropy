import { UserRepository } from "./repository";

export class UserService {
  getDisplayName(id: string): string {
    const repo = new UserRepository();
    return repo.findName(id).toUpperCase();
  }
}
