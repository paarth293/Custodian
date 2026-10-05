/**
 * Exact arithmetic for the `calculate` tool. Small language models get sums
 * wrong surprisingly often, so the model hands arithmetic to this parser
 * instead of doing it itself. It never evaluates code: only numbers,
 * + - * / ^, parentheses and percentages ("17% * 2350").
 */

const MAX_LENGTH = 200;

class Parser {
  private pos = 0;

  constructor(private readonly text: string) {}

  parse(): number {
    const value = this.expression();
    this.skipSpace();
    if (this.pos !== this.text.length) throw new Error(`unexpected "${this.text[this.pos]}"`);
    return value;
  }

  private skipSpace(): void {
    while (this.text[this.pos] === ' ') this.pos++;
  }

  private peek(): string | undefined {
    this.skipSpace();
    return this.text[this.pos];
  }

  private expression(): number {
    let value = this.term();
    for (let op = this.peek(); op === '+' || op === '-'; op = this.peek()) {
      this.pos++;
      const right = this.term();
      value = op === '+' ? value + right : value - right;
    }
    return value;
  }

  private term(): number {
    let value = this.power();
    for (let op = this.peek(); op === '*' || op === '/'; op = this.peek()) {
      this.pos++;
      const right = this.power();
      if (op === '/' && right === 0) throw new Error('division by zero');
      value = op === '*' ? value * right : value / right;
    }
    return value;
  }

  /** Right-associative: 2^3^2 = 2^9. */
  private power(): number {
    const base = this.unary();
    if (this.peek() === '^') {
      this.pos++;
      return base ** this.power();
    }
    return base;
  }

  private unary(): number {
    const op = this.peek();
    if (op === '-' || op === '+') {
      this.pos++;
      const value = this.unary();
      return op === '-' ? -value : value;
    }
    return this.postfix();
  }

  /** A trailing % means "per cent": 17% = 0.17. */
  private postfix(): number {
    let value = this.primary();
    while (this.peek() === '%') {
      this.pos++;
      value /= 100;
    }
    return value;
  }

  private primary(): number {
    if (this.peek() === '(') {
      this.pos++;
      const value = this.expression();
      if (this.peek() !== ')') throw new Error('missing ")"');
      this.pos++;
      return value;
    }
    const match = /^\d+(\.\d+)?|^\.\d+/.exec(this.text.slice(this.pos));
    if (!match) throw new Error(this.pos < this.text.length ? 'expected a number' : 'unfinished');
    this.pos += match[0].length;
    return Number(match[0]);
  }
}

/** Formats a result without floating-point noise: 0.1 + 0.2 gives "0.3". */
function format(value: number): string {
  if (!Number.isFinite(value)) throw new Error('the result is too large');
  const rounded = Number(value.toPrecision(12));
  return Math.abs(rounded) >= 1e15 || (rounded !== 0 && Math.abs(rounded) < 1e-9)
    ? rounded.toExponential()
    : String(rounded);
}

/** Evaluates `expression`, returning the result or a short error message. */
export function calculate(expression: string): string {
  if (expression.length > MAX_LENGTH) return 'Error: the expression is too long.';
  // Common ways of writing operators and numbers ("3 x 4", "1,250", "2 ÷ 5").
  const text = expression
    .replace(/[×xX]/g, '*')
    .replace(/÷/g, '/')
    .replace(/(\d),(?=\d{3}\b)/g, '$1')
    .replace(/\s+/g, ' ')
    .trim();
  if (!/^[\d+\-*/^%(). ]+$/.test(text)) {
    return 'Error: only numbers, + - * / ^ % and parentheses are allowed.';
  }
  try {
    return format(new Parser(text).parse());
  } catch (err) {
    return `Error: ${err instanceof Error ? err.message : String(err)}.`;
  }
}
