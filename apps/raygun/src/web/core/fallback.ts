// What the web core answers for a command it has no implementation of. Reads ("list", "status")
// get an empty answer of their own type, so a screen shows that there's nothing; actions that
// would do something answer an error saying the web version can't, so nothing pretends to work.

/** One command's empty answer, made at build time from the bindings (web-build/commandAnswers). */
export type CommandAnswer = { result: boolean; empty: unknown };
export type CommandAnswers = Record<string, CommandAnswer>;

export const NOT_ON_THE_WEB =
  "This isn't available in Raygun on the web. Use the app on a phone or computer.";

/** Words in a command's name that make it an action, not a read. */
const ACTIONS = new Set([
  'add',
  'answer',
  'apply',
  'assign',
  'cancel',
  'capture',
  'check',
  'clear',
  'connect',
  'convert',
  'copy',
  'create',
  'delete',
  'disconnect',
  'dismiss',
  'download',
  'enqueue',
  'erase',
  'export',
  'follow',
  'forget',
  'grant',
  'hold',
  'import',
  'install',
  'join',
  'keep',
  'link',
  'make',
  'mark',
  'merge',
  'migrate',
  'move',
  'open',
  'pause',
  'pick',
  'pin',
  'play',
  'record',
  'redownload',
  'refresh',
  'remove',
  'rename',
  'reorder',
  'request',
  'rescan',
  'reset',
  'restore',
  'resume',
  'retry',
  'run',
  'save',
  'scan',
  'send',
  'set',
  'share',
  'skip',
  'start',
  'stop',
  'subscribe',
  'sync',
  'transcribe',
  'translate',
  'uninstall',
  'unlink',
  'unpin',
  'update',
  'upload',
  'use',
  'write',
]);

/** Whether a command named `command` (snake case) does something rather than reads. */
export const isAction = (command: string) => command.split('_').some((word) => ACTIONS.has(word));

/**
 * The answer for a command nobody implemented: an empty value, or (for actions answering a
 * `Result`) the error that says it isn't available here. Plain commands (no `Result`) never fail
 * in Rust, so they always get their empty value.
 */
export const fallbackAnswer = (answers: CommandAnswers, command: string): unknown => {
  const answer = answers[command];
  if (!answer) throw NOT_ON_THE_WEB;
  if (answer.result && isAction(command)) throw NOT_ON_THE_WEB;
  return structuredClone(answer.empty);
};
