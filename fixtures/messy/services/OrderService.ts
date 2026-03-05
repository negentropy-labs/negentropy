// Triggers: EAD (Feature Envy — accesses user properties more than own)
import { AuthService } from './AuthService';
import { UserService } from './UserService';

export class OrderService {
  constructor(private auth: AuthService) {}

  calculateDiscount(user: any) {
    // Feature Envy: this method accesses user properties 6 times
    // but never accesses this.xxx
    let discount = 0;
    if (user.tier === 'gold') discount = user.totalSpent * 0.1;
    if (user.age > 60) discount += user.totalSpent * 0.05;
    if (user.referralCount > 5) discount += user.referralBonus;
    return discount;
  }

  processOrder(orderId: string, userId: string) {
    let userService = new UserService();
    let user = userService.getUser(userId);
    return { orderId, user };
  }
}
