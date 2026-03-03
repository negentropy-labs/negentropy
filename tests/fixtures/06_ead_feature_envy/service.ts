import { Order } from "./models";

export function describeOrder(order: Order): string {
  const formattedTotal = `${order.total + order.tax} ${order.currency}`;
  const msg = `Order ${order.id}: ${formattedTotal}`;
  return msg;
}

export function orderBucket(order: Order): string {
  if (order.total > 1000) {
    return "enterprise";
  }
  return "standard";
}
