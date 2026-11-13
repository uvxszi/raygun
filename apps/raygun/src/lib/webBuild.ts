// Whether this page is Raygun on the web (docs/WEB.md): set by the web entry before the app
// starts, so parts drawn many times (catalogue rows) can tell without a query each.
const flag = globalThis as { __RAYGUN_WEB__?: boolean };

export const markWebBuild = () => {
  flag.__RAYGUN_WEB__ = true;
};

export const isWebBuild = () => flag.__RAYGUN_WEB__ === true;
