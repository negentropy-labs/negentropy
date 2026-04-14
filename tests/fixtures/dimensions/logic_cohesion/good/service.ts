export class MathService {
  totalWithTax(subtotal: number, taxRate: number): number {
    const tax = subtotal * taxRate;
    const total = subtotal + tax;
    return total;
  }
}
