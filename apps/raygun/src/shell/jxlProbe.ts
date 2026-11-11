import { commands } from '@raygun/bindings';
import { useEffect } from 'react';

/**
 * An 8×8 JPEG XL picture made as covers are stored: a JPEG recompressed losslessly, in the
 * container with its reconstruction box (raygun-net `cover_format`).
 */
export const JXL_PROBE =
  'data:image/jxl;base64,AAAADEpYTCANCocKAAAAFGZ0eXBqeGwgAAAAAGp4bCAAAAARanhscAAAAAD/CkFAJAAAAeBqYnJkQGDbCIIgtAwIgFCjQMhVJQAAIOSobMFQnP8TRMYIZZxTCtqPkCHMUgcZZqB0hxqsDxlOPmbAIQcVFjlo2IHto4b+/ZFlDjvuQEPR/S/GOGOSSzb5ZJRTnpnmmm2+GeechR6a6KKNPhrppKWemuqqrb4a66zFHpvsss0+G+205Z6b7rrtvhvvvAUfnPDCDT8c8cQjl3xyyiu3/HLMMw9d9NFJL93001FPPXbZZ6e9dttvxz334IUfnvjijT8e+eSjl3566qu3/nrsswc/fPHHJ798889HP33445d/fvrrt/9+/PP/VzBVVQIAIOSobMFQnP8XREIpo6xTisR4QMhm1CDDpDrQUD3YcO4hxwwoyAcNOeiwAxuLLHPU7/Zhxw2t6I88M/gkw/tfjHHGJZt8Msop01yzzTfjnLPQQxNdtNFHI5201FNTXbXVV2Odtdhjk1222Wejnbbcc9Ndt91345134IIPTnjhhh+OeOKRSz455ZVbfjnmmYcu+uikl2766ainHrvss9Neu+2345578MIPT3zxxh+PfPLRSz899dVbfz322Ycv/vjkl2/++einH7/889Nfv/3345//v8APAFQWAAsIgOAAEEpGSUYAAQIAAAEAAQAAAwAAAVpqeGxwgAAAAeBbABAAHAWIeAh4iHj+/w4AAEASY5/ctqqq/h/A/wMAAAAAACBgSCKUehIk1jki6GT/s1uMYpIkSaLQOvMbnL56pbh60eHtFfgBQI/fsLm7r6/3Qbp8m+tprbN7+w3XNlzrR/f63b7T291qS61satTWju7qPX3fQ0JCQkJAQEAAAAAAAAAAAAAAFQAAAAAAAAAAAPSfAQBAxGgOk/UlADCvYeCqNQcUgJwAQuSXChXEyLUiVK+xJ+l6kYqKioo9R8WLMSqqAhSwUgBQFaDWugJApRZAVZXrVwAANRv/z0uhPYWRL3gxRmK4n1fPQrMPJ/wq/8wEwWP926+xgkPKqhjk8hjVfjZKLq0FTmeUcfunJujTeDLjwf7E484DzfB0uK81p12rYsOHisAl70HgCB04AAAAAAAAihjNfsUMAYDtKexd5kUA69sOb6AvDwA=';

/**
 * Whether this webview shows JPEG XL pictures: it shows the probe picture, or fails to. Until it
 * answers, Rust takes it as a no.
 */
export const showsJxl = (): Promise<boolean> =>
  new Promise((resolve) => {
    const picture = new Image();
    picture.onload = () => resolve(picture.naturalWidth === 8);
    picture.onerror = () => resolve(false);
    picture.src = JXL_PROBE;
  });

/**
 * Tells Rust once per start whether the webview shows JPEG XL, so covers stored as JPEG XL reach
 * it as they are (else rebuilt as JPEG or PNG). Asked at every start: webviews update on their own.
 */
export const useJxlProbe = () => {
  useEffect(() => {
    let current = true;
    void showsJxl().then((shows) => {
      if (current) void commands.coversWebviewJxl(shows);
    });
    return () => {
      current = false;
    };
  }, []);
};
