// Triggers: PLME (deep relative imports), SSE (let not reassigned), EDR (hardcoded new)
import { Database } from '../../../utils/database';
import { Logger } from '../../../../utils/logger';
import { UserModel } from '../../models/UserModel';
import { AuthService } from './AuthService'; // circular: UserService <-> AuthService

export class UserService {
  constructor() {
    // No dependency injection — everything hardcoded
  }

  async getUser(id: string) {
    let db = new Database();
    let logger = new Logger();
    let result = db.query(`SELECT * FROM users WHERE id = ${id}`);
    logger.log("fetched user");
    return result;
  }

  async updateUser(user: UserModel) {
    let db = new Database();
    // Mutating external object directly (OA trigger)
    user.updatedAt = new Date();
    user.status = 'active';
    return db.save(user);
  }
}
