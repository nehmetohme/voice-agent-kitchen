/**
 * Helpers for the class-name rules: find the string literals that hold Tailwind classes, and
 * split a class into its variants and its utility.
 *
 * Class strings are looked for in `className` JSX attributes, in `className` object properties
 * (for example TanStack Router's `activeProps`), and in calls to the class helpers below.
 */

export const CLASS_HELPERS = new Set(['cn', 'cva', 'clsx', 'twMerge']);

/** The few AST fields these helpers read. `range` lets a node be passed to `context.report`. */
export interface AstNode {
  type: string;
  range: [number, number];
  [key: string]: unknown;
}

export interface ClassString {
  node: AstNode;
  value: string;
}

export function isNode(value: unknown): value is AstNode {
  return (
    typeof value === 'object' && value !== null && typeof Reflect.get(value, 'type') === 'string'
  );
}

/** The child node under `key`, if it is a node. */
export function child(node: AstNode, key: string): AstNode | undefined {
  const value = node[key];
  return isNode(value) ? value : undefined;
}

/** The child nodes under `key`, whether it holds one node or a list. */
export function childList(node: AstNode, key: string): AstNode[] {
  const value = node[key];
  const list: unknown[] = Array.isArray(value) ? value : [value];
  return list.filter(isNode);
}

/** True for a call such as `cn(...)`: those are collected by their own visitor. */
export function isClassHelperCall(node: AstNode): boolean {
  const callee = child(node, 'callee');
  const name = callee?.['name'];
  return (
    node.type === 'CallExpression' &&
    callee?.type === 'Identifier' &&
    typeof name === 'string' &&
    CLASS_HELPERS.has(name)
  );
}

function textOf(node: AstNode): string | undefined {
  const value = node['value'];
  if (node.type === 'Literal') return typeof value === 'string' ? value : undefined;
  if (node.type !== 'TemplateElement' || typeof value !== 'object' || value === null)
    return undefined;
  const cooked: unknown = Reflect.get(value, 'cooked');
  return typeof cooked === 'string' ? cooked : undefined;
}

const WALK_KEYS: Record<string, string[]> = {
  JSXExpressionContainer: ['expression'],
  ConditionalExpression: ['consequent', 'alternate'],
  LogicalExpression: ['left', 'right'],
  ArrayExpression: ['elements'],
  ObjectExpression: ['properties'],
  Property: ['key', 'value'],
  CallExpression: ['arguments'],
  TemplateLiteral: ['quasis'],
};

/** Collects the string parts of an expression that can end up in a class list. */
export function collectClassStrings(start: unknown, out: ClassString[] = []): ClassString[] {
  if (!isNode(start)) return out;
  const text = textOf(start);
  if (text !== undefined) {
    out.push({ node: start, value: text });
    return out;
  }
  for (const key of WALK_KEYS[start.type] ?? []) {
    for (const node of childList(start, key)) {
      if (!isClassHelperCall(node)) collectClassStrings(node, out);
    }
  }
  return out;
}

/** Splits `hover:md:bg-primary/50` into its utility `bg-primary/50`, ignoring `:` inside brackets. */
export function utilityOf(className: string): string {
  let depth = 0;
  let start = 0;
  for (let index = 0; index < className.length; index += 1) {
    const char = className[index];
    if (char === '[' || char === '(') depth += 1;
    else if (char === ']' || char === ')') depth -= 1;
    else if (char === ':' && depth === 0) start = index + 1;
  }
  return className.slice(start).replace(/^!|!$/g, '');
}

export function classTokens(value: string): string[] {
  return value.split(/\s+/).filter(Boolean);
}
