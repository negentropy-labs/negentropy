import { Customer } from "./model";

export function evaluate(customer: Customer): number {
  const a = customer.age;
  const b = customer.orders;
  const c = customer.balance;
  const d = customer.risk;
  const e = customer.tickets;
  const f = customer.score;
  return a + b + c + d + e + f;
}
