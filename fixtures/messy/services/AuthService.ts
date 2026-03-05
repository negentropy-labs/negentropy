// Triggers: TCE (circular dependency with UserService)
import { UserService } from './UserService';

export class AuthService {
  private userService: UserService;

  constructor() {
    this.userService = new UserService();
  }

  async authenticate(token: string) {
    let decoded = this.parseToken(token);
    let user = await this.userService.getUser(decoded.id);
    return user;
  }

  parseToken(token: string) {
    return { id: token.split('.')[1] };
  }
}
