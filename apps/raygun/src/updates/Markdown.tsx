import { useMemo } from 'react';
import type { FC, ReactNode } from 'react';

import { parseInline, parseMarkdown } from './markdownParse';
import type { MarkdownBlock, MarkdownInline } from './markdownParse';

/** Release notes, rendered from Markdown without HTML (see `markdownParse.ts`). */
type RenderProps = { onOpenLink: (url: string) => void };

const Inline: FC<RenderProps & { text: string }> = ({ text, onOpenLink }) => (
  <>{renderInline(parseInline(text), onOpenLink)}</>
);

const renderInline = (nodes: MarkdownInline[], onOpenLink: (url: string) => void): ReactNode[] =>
  nodes.map((node, position) => {
    switch (node.kind) {
      case 'text':
        return node.text;
      case 'break':
        return <br key={position} />;
      case 'code':
        return (
          <code key={position} className="bg-background rounded px-1 py-0.5 font-mono text-[0.9em]">
            {node.text}
          </code>
        );
      case 'strong':
        return (
          <strong key={position} className="text-foreground font-semibold">
            {renderInline(node.children, onOpenLink)}
          </strong>
        );
      case 'em':
        return <em key={position}>{renderInline(node.children, onOpenLink)}</em>;
      case 'del':
        return <del key={position}>{renderInline(node.children, onOpenLink)}</del>;
      case 'link': {
        const { url } = node;
        if (!url) return <span key={position}>{renderInline(node.children, onOpenLink)}</span>;
        return (
          <a
            key={position}
            href={url}
            target="_blank"
            rel="noreferrer noopener"
            onClick={(event) => {
              event.preventDefault();
              onOpenLink(url);
            }}
            className="text-primary underline-offset-2 hover:underline"
          >
            {renderInline(node.children, onOpenLink)}
          </a>
        );
      }
    }
  });

const HEADING_CLASS = [
  'text-xl font-bold',
  'text-lg font-semibold',
  'text-base font-semibold',
  'text-sm font-semibold',
  'text-sm font-semibold',
  'text-muted text-sm font-semibold',
];

const Blocks: FC<RenderProps & { blocks: MarkdownBlock[]; tight?: boolean }> = ({
  blocks,
  tight,
  onOpenLink,
}) => (
  <>
    {blocks.map((block, position) => {
      switch (block.kind) {
        case 'heading': {
          const Tag = `h${Math.min(block.level + 1, 6)}` as 'h2';
          return (
            <Tag
              key={position}
              className={`${HEADING_CLASS[block.level - 1]} text-foreground mt-2`}
            >
              <Inline text={block.text} onOpenLink={onOpenLink} />
            </Tag>
          );
        }
        case 'paragraph':
          return tight ? (
            <span key={position} className="block">
              <Inline text={block.text} onOpenLink={onOpenLink} />
            </span>
          ) : (
            <p key={position}>
              <Inline text={block.text} onOpenLink={onOpenLink} />
            </p>
          );
        case 'code':
          return (
            <pre
              key={position}
              className="bg-background overflow-x-auto rounded-xl px-3 py-2 font-mono text-xs whitespace-pre"
            >
              <code>{block.text}</code>
            </pre>
          );
        case 'list': {
          const items = block.items.map((item, itemPosition) => (
            <li key={itemPosition} className="flex flex-col gap-1">
              <span className="flex gap-2">
                {item.checked !== null && (
                  <input
                    type="checkbox"
                    checked={item.checked}
                    readOnly
                    disabled
                    className="mt-1"
                  />
                )}
                <span className="flex min-w-0 flex-1 flex-col gap-1">
                  <Blocks blocks={item.blocks} tight onOpenLink={onOpenLink} />
                </span>
              </span>
            </li>
          ));
          return block.ordered ? (
            <ol
              key={position}
              start={block.start}
              className="flex list-decimal flex-col gap-1 pl-5"
            >
              {items}
            </ol>
          ) : (
            <ul key={position} className="flex list-disc flex-col gap-1 pl-5">
              {items}
            </ul>
          );
        }
        case 'quote':
          return (
            <blockquote
              key={position}
              className="border-muted/40 flex flex-col gap-2 border-l-2 pl-3"
            >
              <Blocks blocks={block.blocks} onOpenLink={onOpenLink} />
            </blockquote>
          );
        case 'table':
          return (
            <div key={position} className="overflow-x-auto">
              <table className="text-left text-sm">
                <thead>
                  <tr>
                    {block.head.map((cell, cellPosition) => (
                      <th key={cellPosition} className="px-2 py-1 font-semibold">
                        <Inline text={cell} onOpenLink={onOpenLink} />
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {block.rows.map((row, rowPosition) => (
                    <tr key={rowPosition} className="border-muted/20 border-t">
                      {row.map((cell, cellPosition) => (
                        <td key={cellPosition} className="px-2 py-1">
                          <Inline text={cell} onOpenLink={onOpenLink} />
                        </td>
                      ))}
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          );
        case 'rule':
          return <hr key={position} className="border-muted/30" />;
      }
    })}
  </>
);

type MarkdownProps = RenderProps & { source: string };

export const Markdown: FC<MarkdownProps> = ({ source, onOpenLink }) => {
  const blocks = useMemo(() => parseMarkdown(source), [source]);
  return (
    <div data-testid="markdown" className="selectable flex flex-col gap-3 text-sm leading-relaxed">
      <Blocks blocks={blocks} onOpenLink={onOpenLink} />
    </div>
  );
};
