export class UserRepository {
  findName(id: string): string {
    return `user-${id}`;
  }
}
