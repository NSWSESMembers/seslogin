// Registers toBeInTheDocument and friends for every test file. Without it each
// file has to import it itself, and one that forgets fails with the unhelpful
// "Invalid Chai property: toBeInTheDocument" rather than anything pointing at
// the missing import.
import "@testing-library/jest-dom/vitest";

/**
 * Installs a global for every test, the way `vi.stubGlobal` does but permanently:
 * a test's own `vi.unstubAllGlobals()` would otherwise remove it too. What's left
 * underneath depends on Node: from Node 25 `localStorage` is Node's own, which is
 * undefined without `--localstorage-file`.
 */
function installGlobal(name: string, value: unknown) {
  const descriptor = { value, writable: true, configurable: true };
  Object.defineProperty(globalThis, name, descriptor);
  if (typeof window !== "undefined" && window !== globalThis) {
    Object.defineProperty(window, name, descriptor);
  }
}

const localStorageMock = (() => {
  let store: Record<string, string> = {};
  return {
    getItem: (key: string) => store[key] || null,
    setItem: (key: string, value: string) => {
      store[key] = value.toString();
    },
    removeItem: (key: string) => {
      delete store[key];
    },
    clear: () => {
      store = {};
    },
    key: (index: number) => Object.keys(store)[index] || null,
    get length() {
      return Object.keys(store).length;
    },
  };
})();

installGlobal("localStorage", localStorageMock);

// jsdom implements neither of these. `matchMedia` resolves a `(min-width: …)`
// query against `window.innerWidth` (1024 by default), so a test wanting the
// small-screen branch of a responsive component sets `window.innerWidth` before
// rendering. Only `min-width` in px or rem is understood — enough for the
// Tailwind breakpoints this app queries.
installGlobal("matchMedia", (query: string) => {
  const match = /min-width:\s*([\d.]+)(px|rem)/.exec(query);
  const minWidth = match
    ? Number(match[1]) * (match[2] === "rem" ? 16 : 1)
    : Number.POSITIVE_INFINITY;
  return {
    matches: window.innerWidth >= minWidth,
    media: query,
    onchange: null,
    addEventListener: () => {},
    removeEventListener: () => {},
    addListener: () => {},
    removeListener: () => {},
    dispatchEvent: () => false,
  };
});

if (typeof Element !== "undefined") {
  Element.prototype.scrollIntoView = function () {};
}

if (typeof HTMLMediaElement !== "undefined") {
  HTMLMediaElement.prototype.play = function () {
    return Promise.resolve();
  };

  HTMLMediaElement.prototype.pause = function () {};
  HTMLMediaElement.prototype.load = function () {};
}
