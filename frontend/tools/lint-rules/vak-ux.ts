/**
 * `vak-ux`: Oxlint JS plugin with the lint rules behind the UX laws in docs/ux-guardrails/.
 *
 * The rules do not know about folders. Where a rule does not apply (for example, shared/ui may
 * render a raw <button>), .oxlintrc.json turns it off with an `overrides` entry.
 */
import type { RuleTester } from 'oxlint/plugins-dev';

import {
  child,
  childList,
  classTokens,
  collectClassStrings,
  isClassHelperCall,
  isNode,
  utilityOf,
  type AstNode,
  type ClassString,
} from './class-names.ts';

type Rule = Parameters<RuleTester['run']>[1];
type Visitor = ReturnType<Extract<Rule, { create: unknown }>['create']>;

const BUTTON_INPUT_TYPES = new Set(['button', 'submit', 'reset']);

function staticAttribute(element: AstNode, name: string): string | undefined {
  for (const attribute of childList(element, 'attributes')) {
    const value = child(attribute, 'value');
    if (child(attribute, 'name')?.['name'] === name && value?.type === 'Literal') {
      return String(value['value']);
    }
  }
  return undefined;
}

/** Similarity, Von Restorff, Fitts: every button comes from shared/ui. */
const noRawButton: Rule = {
  meta: {
    type: 'problem',
    docs: { description: 'Use <Button> from shared/ui instead of a raw <button>.' },
    messages: {
      rawButton:
        'Use <Button> from shared/ui instead of a raw {{element}}. It carries data-variant, the 44 px target and the focus styles.',
    },
  },
  create(context) {
    return {
      JSXOpeningElement(node) {
        if (node.name.type !== 'JSXIdentifier') return;
        const type = isNode(node) ? staticAttribute(node, 'type') : undefined;
        if (node.name.name === 'button') {
          context.report({ node, messageId: 'rawButton', data: { element: '<button>' } });
        } else if (
          node.name.name === 'input' &&
          type !== undefined &&
          BUTTON_INPUT_TYPES.has(type)
        ) {
          context.report({
            node,
            messageId: 'rawButton',
            data: { element: `<input type="${type}">` },
          });
        }
      },
    };
  },
};

/**
 * Visits every class string exactly once: in `className` attributes and properties, and in
 * calls to the class helpers (`cn`, `cva`, ...). A helper call inside `className` is left to
 * the call visitor, so it is not reported twice.
 */
function classVisitor(check: (found: ClassString, className: string) => void): Visitor {
  const visit = (start: unknown, fromHelper = false) => {
    if (!fromHelper && isNode(start) && isClassHelperCall(start)) return;
    for (const found of collectClassStrings(start)) {
      for (const className of classTokens(found.value)) check(found, className);
    }
  };
  return {
    JSXAttribute(node) {
      if (node.name.type === 'JSXIdentifier' && node.name.name === 'className') {
        const value = node.value;
        visit(value?.type === 'JSXExpressionContainer' ? value.expression : value);
      }
    },
    Property(node) {
      const key = node.key;
      const named =
        (key.type === 'Identifier' && key.name === 'className') ||
        (key.type === 'Literal' && key.value === 'className');
      if (named) visit(node.value);
    },
    CallExpression(node) {
      if (isNode(node) && isClassHelperCall(node)) visit(node, true);
    },
  };
}

/** Proximity, Prägnanz: spacing, colour and sizes come from the token scale only. */
const noArbitraryValues: Rule = {
  meta: {
    type: 'problem',
    docs: { description: 'Disallow arbitrary Tailwind values such as p-[13px] or bg-(--x).' },
    messages: {
      arbitrary:
        '"{{className}}" uses an arbitrary value. Use a token from tokens/*.tokens.json, or add one.',
    },
  },
  create(context) {
    return classVisitor(({ node }, className) => {
      const utility = utilityOf(className);
      if (utility.includes('[') || utility.includes('(')) {
        context.report({ node, messageId: 'arbitrary', data: { className } });
      }
    });
  },
};

const PRIMARY_COLOUR = /^[a-z]+(?:-[a-z]+)*?-(?:on-)?primary(?:-[a-z0-9]+)*(?:\/\d+)?$/;

/** Von Restorff: the primary colour is reserved for the one primary call to action. */
const primaryColourOnlyInButton: Rule = {
  meta: {
    type: 'problem',
    docs: { description: 'Only shared/ui/button.tsx may use the primary colour tokens.' },
    messages: {
      primary:
        '"{{className}}" uses the primary colour, which is reserved for <Button variant="primary">.',
    },
  },
  create(context) {
    return classVisitor(({ node }, className) => {
      if (PRIMARY_COLOUR.test(utilityOf(className))) {
        context.report({ node, messageId: 'primary', data: { className } });
      }
    });
  },
};

export default {
  meta: { name: 'vak-ux' },
  rules: {
    'no-raw-button': noRawButton,
    'no-arbitrary-values': noArbitraryValues,
    'primary-colour-only-in-button': primaryColourOnlyInButton,
  },
};
