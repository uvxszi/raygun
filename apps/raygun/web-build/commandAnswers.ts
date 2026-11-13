// Reads the generated bindings (packages/bindings/src/index.ts) with the TypeScript compiler and
// gives every command an empty answer of its own type: [] for lists, null where null is allowed,
// false, 0 and '' for scalars, the first member of a union, objects field by field. Raygun on the
// web answers the commands it has no implementation for with these, so screens draw "nothing
// here" instead of failing on a missing field.
import ts from 'typescript';

/** One command: whether it answers a `Result` (`typedError` in the bindings), and its empty value. */
export type CommandAnswer = { result: boolean; empty: unknown };

const MAX_DEPTH = 14;

const isNullish = (type: ts.Type) =>
  (type.flags & (ts.TypeFlags.Null | ts.TypeFlags.Undefined | ts.TypeFlags.Void)) !== 0;

const emptyOf = (
  checker: ts.TypeChecker,
  type: ts.Type,
  depth: number,
  seen: Set<ts.Type>,
): unknown => {
  if (depth > MAX_DEPTH) return null;
  const flags = type.flags;
  if (flags & (ts.TypeFlags.Any | ts.TypeFlags.Unknown | ts.TypeFlags.Never)) return null;
  if (isNullish(type)) return null;
  if (type.isStringLiteral()) return type.value;
  if (type.isNumberLiteral()) return type.value;
  if (flags & ts.TypeFlags.BooleanLiteral) {
    return (type as ts.Type & { intrinsicName?: string }).intrinsicName === 'true';
  }
  if (flags & ts.TypeFlags.StringLike) return '';
  if (flags & ts.TypeFlags.NumberLike) return 0;
  if (flags & ts.TypeFlags.BigIntLike) return 0;
  if (flags & ts.TypeFlags.BooleanLike) return false;
  if (type.isUnion()) {
    const members = type.types;
    if (members.some(isNullish)) return null;
    // `boolean` is `true | false`.
    if (members.every((member) => member.flags & ts.TypeFlags.BooleanLiteral)) return false;
    const first = members[0];
    return first ? emptyOf(checker, first, depth + 1, seen) : null;
  }
  if (checker.isArrayType(type)) return [];
  if (checker.isTupleType(type)) {
    const elements = checker.getTypeArguments(type as ts.TypeReference);
    return elements.map((element) => emptyOf(checker, element, depth + 1, seen));
  }
  if (type.isIntersection()) {
    return Object.assign(
      {},
      ...type.types.map((part) => {
        const value = emptyOf(checker, part, depth + 1, seen);
        return value && typeof value === 'object' ? value : {};
      }),
    );
  }
  if (flags & ts.TypeFlags.Object) {
    if (seen.has(type)) return null;
    seen.add(type);
    const value: Record<string, unknown> = {};
    for (const property of checker.getPropertiesOfType(type)) {
      // Optional fields stay out, as Rust leaves them out.
      if (property.flags & ts.SymbolFlags.Optional) continue;
      const declaration = property.valueDeclaration ?? property.declarations?.[0];
      if (!declaration) continue;
      const propertyType = checker.getTypeOfSymbolAtLocation(property, declaration);
      value[property.name] = emptyOf(checker, propertyType, depth + 1, seen);
    }
    seen.delete(type);
    return value;
  }
  return null;
};

/** The command a bindings entry calls: `__TAURI_INVOKE<T>("name", …)` or `typedError<T, E>(…)`. */
const readEntry = (
  body: ts.Node,
): { command: string; result: boolean; returned: ts.TypeNode | undefined } | null => {
  let found: { command: string; result: boolean; returned: ts.TypeNode | undefined } | null = null;
  const visit = (node: ts.Node) => {
    if (found) return;
    if (ts.isCallExpression(node) && ts.isIdentifier(node.expression)) {
      const callee = node.expression.text;
      if (callee === 'typedError') {
        const inner = node.arguments[0];
        if (inner && ts.isCallExpression(inner)) {
          const name = inner.arguments[0];
          if (name && ts.isStringLiteral(name)) {
            found = { command: name.text, result: true, returned: node.typeArguments?.[0] };
            return;
          }
        }
      }
      if (callee === '__TAURI_INVOKE') {
        const name = node.arguments[0];
        if (name && ts.isStringLiteral(name)) {
          found = { command: name.text, result: false, returned: node.typeArguments?.[0] };
          return;
        }
      }
    }
    ts.forEachChild(node, visit);
  };
  visit(body);
  return found;
};

/** Every command in the bindings file at `bindingsPath`, by its Rust name. */
export const commandAnswers = (bindingsPath: string): Record<string, CommandAnswer> => {
  const program = ts.createProgram([bindingsPath], {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    strict: true,
    skipLibCheck: true,
    noEmit: true,
    types: [],
  });
  const checker = program.getTypeChecker();
  const source = program.getSourceFile(bindingsPath);
  if (!source) throw new Error(`no bindings at ${bindingsPath}`);
  const answers: Record<string, CommandAnswer> = {};
  ts.forEachChild(source, (statement) => {
    if (!ts.isVariableStatement(statement)) return;
    for (const declaration of statement.declarationList.declarations) {
      if (!ts.isIdentifier(declaration.name) || declaration.name.text !== 'commands') continue;
      const literal = declaration.initializer;
      if (!literal || !ts.isObjectLiteralExpression(literal)) continue;
      for (const property of literal.properties) {
        if (!ts.isPropertyAssignment(property)) continue;
        const entry = readEntry(property.initializer);
        if (!entry) continue;
        const empty = entry.returned
          ? emptyOf(checker, checker.getTypeFromTypeNode(entry.returned), 0, new Set())
          : null;
        answers[entry.command] = { result: entry.result, empty };
      }
    }
  });
  return answers;
};
