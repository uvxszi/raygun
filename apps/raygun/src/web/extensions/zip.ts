// Reads extension packages (zip files) in the browser: the central directory, then each entry
// stored or deflated, inflated with the browser's own DecompressionStream.

const END_OF_DIRECTORY = 0x06054b50;
const DIRECTORY_ENTRY = 0x02014b50;
const LOCAL_ENTRY = 0x04034b50;

const inflate = async (data: Uint8Array): Promise<Uint8Array> => {
  const stream = new ReadableStream<Uint8Array<ArrayBuffer>>({
    start: (controller) => {
      controller.enqueue(data.slice());
      controller.close();
    },
  }).pipeThrough(new DecompressionStream('deflate-raw'));
  return new Uint8Array(await new Response(stream).arrayBuffer());
};

/** Every file in a zip archive, by its path. */
export const readZip = async (archive: Uint8Array): Promise<Map<string, Uint8Array>> => {
  const view = new DataView(archive.buffer, archive.byteOffset, archive.byteLength);
  let end = -1;
  // The end record sits in the last 64 KiB (after an optional comment).
  for (let at = archive.length - 22; at >= Math.max(0, archive.length - 65_557); at -= 1) {
    if (view.getUint32(at, true) === END_OF_DIRECTORY) {
      end = at;
      break;
    }
  }
  if (end < 0) throw new Error("that package isn't a zip archive");
  const count = view.getUint16(end + 10, true);
  let at = view.getUint32(end + 16, true);
  const files = new Map<string, Uint8Array>();
  const decoder = new TextDecoder();
  for (let index = 0; index < count; index += 1) {
    if (view.getUint32(at, true) !== DIRECTORY_ENTRY) throw new Error('a damaged zip archive');
    const method = view.getUint16(at + 10, true);
    const compressed = view.getUint32(at + 20, true);
    const nameLength = view.getUint16(at + 28, true);
    const extraLength = view.getUint16(at + 30, true);
    const commentLength = view.getUint16(at + 32, true);
    const local = view.getUint32(at + 42, true);
    const name = decoder.decode(archive.subarray(at + 46, at + 46 + nameLength));
    at += 46 + nameLength + extraLength + commentLength;
    if (name.endsWith('/')) continue;
    if (view.getUint32(local, true) !== LOCAL_ENTRY) throw new Error('a damaged zip archive');
    const start = local + 30 + view.getUint16(local + 26, true) + view.getUint16(local + 28, true);
    const data = archive.subarray(start, start + compressed);
    if (method === 0) files.set(name, data.slice());
    else if (method === 8) files.set(name, await inflate(data));
    else throw new Error(`${name} is packed in a way Raygun can't read`);
  }
  return files;
};
