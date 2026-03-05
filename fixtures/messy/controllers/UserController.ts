// Triggers: IIE (shallow wrapper — just delegates to service)
import { UserService } from '../services/UserService';

export function getUser(id: string) {
  return new UserService().getUser(id);
}

export function updateUser(user: any) {
  return new UserService().updateUser(user);
}
