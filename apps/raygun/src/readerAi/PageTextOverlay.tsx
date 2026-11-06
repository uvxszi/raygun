import type { PageTextBlock } from '@raygun/bindings';
import { useLayoutEffect, useRef, useState } from 'react';
import type { FC } from 'react';

type Box = { left: number; top: number; width: number; height: number };

/** Where the picture is in its positioned parent. */
const boxOf = (image: HTMLImageElement): Box => ({
  left: image.offsetLeft,
  top: image.offsetTop,
  width: image.offsetWidth,
  height: image.offsetHeight,
});

/** One block's words, as large as fits its place on the page. */
const FittedText: FC<{ block: PageTextBlock }> = ({ block }) => {
  const ref = useRef<HTMLDivElement>(null);
  const [left, top, right, bottom] = [
    block.left ?? 0,
    block.top ?? 0,
    block.right ?? 0,
    block.bottom ?? 0,
  ];
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    let size = Math.max(8, Math.min(40, element.clientHeight * 0.6));
    element.style.fontSize = `${size}px`;
    while (
      size > 7 &&
      (element.scrollHeight > element.clientHeight + 1 ||
        element.scrollWidth > element.clientWidth + 1)
    ) {
      size -= 1;
      element.style.fontSize = `${size}px`;
    }
  });
  return (
    <div
      ref={ref}
      className="absolute flex items-center justify-center overflow-hidden rounded-md bg-white/95 px-0.5 text-center leading-tight font-semibold text-black"
      style={{
        left: `${left * 100}%`,
        top: `${top * 100}%`,
        // A little room around the words the page had.
        width: `${(right - left) * 100 + 1}%`,
        height: `${(bottom - top) * 100 + 0.6}%`,
      }}
    >
      {block.text}
    </div>
  );
};

/**
 * A translation layer's words over a comic page, each where the page had its own: a box per
 * speech bubble or caption, over the picture wherever it sits in its parent.
 */
export const PageTextOverlay: FC<{ image: HTMLImageElement | null; blocks: PageTextBlock[] }> = ({
  image,
  blocks,
}) => {
  const [box, setBox] = useState<Box | null>(null);
  useLayoutEffect(() => {
    if (!image) return;
    const measure = () => setBox(image.offsetWidth > 0 ? boxOf(image) : null);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(image);
    if (image.parentElement) observer.observe(image.parentElement);
    image.addEventListener('load', measure);
    return () => {
      observer.disconnect();
      image.removeEventListener('load', measure);
    };
  }, [image]);
  if (!box || blocks.length === 0) return null;
  return (
    <div
      data-testid="page-translation"
      className="pointer-events-none absolute"
      style={{ left: box.left, top: box.top, width: box.width, height: box.height }}
    >
      {blocks.map((block, index) => (
        <FittedText key={index} block={block} />
      ))}
    </div>
  );
};
