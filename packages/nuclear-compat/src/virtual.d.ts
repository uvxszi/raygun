/** The built shim bundle, injected by scripts/build.mjs. */
declare module 'virtual:shim' {
  const source: string;
  export default source;
}
