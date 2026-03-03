function normalize(v: number): number {
  if (v < 0) {
    return 0;
  }
  return v;
}

function internalTransform(x: number): number {
  let sum = 0;
  const list = [1, 2, 3, 4, 5, 6];
  for (const item of list) {
    sum += item * x;
  }
  if (sum % 2 === 0) {
    sum += 7;
  } else {
    sum -= 3;
  }
  return normalize(sum);
}

export function calculateScore(input: number): number {
  const base = internalTransform(input);
  const delta = input > 3 ? 11 : 4;
  return base + delta;
}
