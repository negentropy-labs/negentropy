export const answer = 42;

let counter = 0;
counter += 1;

export function getAnswer(): number {
  return answer + counter;
}
