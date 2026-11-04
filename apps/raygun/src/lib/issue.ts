import type { TFunction } from 'i18next';

/**
 * What went wrong when a source failed, in words a listener understands (M46.48). Rust sends a
 * failure as one short sentence, then its raw details on the following lines; older or other
 * errors come as one line, which is sorted by what it names (a status, a timeout, a failed
 * lookup, a script error) into the same kinds as `raygun_net::issue`.
 */
export type IssueKind =
  | 'offline'
  | 'dns'
  | 'timeout'
  | 'tls'
  | 'refused'
  | 'unreachable'
  | 'bot-check'
  | 'denied'
  | 'not-found'
  | 'rate-limited'
  | 'server-error'
  | 'site-changed'
  | 'extension-failed'
  | 'storage'
  | 'permission';

export type Issue = {
  /** The short sentence to show. */
  summary: string;
  /** The raw details for "More info"; null when the summary is all there is. */
  details: string | null;
  /** The kind, when the text told it. */
  kind: IssueKind | null;
};

const statusKind = (status: number): IssueKind | null => {
  if ([408, 504, 522, 524].includes(status)) return 'timeout';
  if (status === 404 || status === 410) return 'not-found';
  if (status === 429) return 'rate-limited';
  if (status >= 500 && status < 600) return 'server-error';
  if (status >= 400 && status < 500) return 'denied';
  return null;
};

const CHECK =
  /cloudflare|captcha|browser check|just a moment|challenge-platform|ddos-guard|checking your browser/i;
const STATUS = /\b(?:http(?:\/[\d.]+)?|answered|status(?: code)?:?|code)\s+([45]\d\d)(?!\d)/i;
const LIMIT = /rate.?limit|too many requests/i;
const DNS =
  /dns error|failed to lookup address|no such host is known|name or service not known|nodename nor servname|no address associated with hostname|temporary failure in name resolution/i;
const TIMEOUT = /timed out|timeout|didn[’']t answer in time|did not answer in time/i;
const REFUSED = /connection refused|actively refused/i;
const TLS = /certificate|\btls\b|\bssl\b|handshake/i;
const OFFLINE = /network is unreachable|network unreachable|no internet/i;
const UNREACHABLE =
  /error sending request|could not reach|couldn[’']t reach|connection reset|connection closed|error trying to connect/i;
const SCRIPT =
  /TypeError|ReferenceError|SyntaxError|RangeError|InternalError|is not a function|is not defined|is not an object|cannot read propert|of undefined|of null|unexpected token|attempt to index|attempt to call/i;
const PERMISSION = /permission denied|access is denied|not permitted|EACCES/i;
const STORAGE = /no space left|disk full|read-only file system|os error 28\b|ENOSPC/i;

/** The kind a one-line error points at; null for a plain sentence that says it all. */
export const issueKind = (text: string): IssueKind | null => {
  if (CHECK.test(text)) return 'bot-check';
  const status = STATUS.exec(text);
  if (status) return statusKind(Number(status[1]));
  if (LIMIT.test(text)) return 'rate-limited';
  if (DNS.test(text)) return 'dns';
  if (TIMEOUT.test(text)) return 'timeout';
  if (REFUSED.test(text)) return 'refused';
  if (OFFLINE.test(text)) return 'offline';
  if (UNREACHABLE.test(text)) return 'unreachable';
  if (TLS.test(text)) return 'tls';
  if (STORAGE.test(text)) return 'storage';
  if (PERMISSION.test(text)) return 'permission';
  if (SCRIPT.test(text)) return 'extension-failed';
  return null;
};

/** Who a one-line error is about, when it says: "Deezer answered HTTP 503", "HTTP 404 from https://x". */
const namedSource = (text: string): string | null => {
  const answered = /^([^:\n]{1,40}?) answered\b/.exec(text.trim());
  if (answered?.[1]) return answered[1];
  const from = /\bfrom (https?:\/\/[^\s)]+)/i.exec(text);
  if (from?.[1]) {
    try {
      return new URL(from[1]).hostname;
    } catch {
      return null;
    }
  }
  return null;
};

/**
 * An error text as a summary and its details. `source` is the source's name when the page knows
 * it: "MangaDex didn't answer in time" rather than "The source didn't answer in time".
 */
export const readIssue = (raw: string, t: TFunction, source?: string | null): Issue => {
  const text = raw.trim();
  const newline = text.indexOf('\n');
  if (newline > 0) {
    // Rust's form: the sentence, then the details.
    const details = text.slice(newline + 1).trim();
    return { summary: text.slice(0, newline).trim(), details: details || null, kind: null };
  }
  const kind = issueKind(text);
  if (!kind) {
    // The source's own words, with its name unless they hold it already.
    const lower = text.toLowerCase();
    // "This source can't search" says who already.
    const named = !source || lower.includes(source.toLowerCase()) || lower.includes('source');
    return { summary: named ? text : `${source}: ${text}`, details: null, kind: null };
  }
  const name = source || namedSource(text) || t('issue.someSource');
  const sentence = t(`issue.kind.${kind}`, { source: name });
  // "The server" leads a sentence; within one it's "the server".
  const summary =
    name.startsWith('The ') && !sentence.startsWith(name)
      ? sentence.replace(name, `t${name.slice(1)}`)
      : sentence;
  return { summary, details: text, kind };
};

/** The sentence to show for an error, its details left out (for places with room for one line). */
export const issueSummary = (raw: string, t: TFunction, source?: string | null) =>
  readIssue(raw, t, source).summary;
