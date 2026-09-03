// Registers toBeInTheDocument and friends for every test file. Without this each
// test file has to import it itself, and the ones that forget fail with the
// unhelpful "Invalid Chai property: toBeInTheDocument".
import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

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

vi.stubGlobal("localStorage", localStorageMock);

if (typeof HTMLMediaElement !== "undefined") {
  HTMLMediaElement.prototype.play = function () {
    return Promise.resolve();
  };

  HTMLMediaElement.prototype.pause = function () {};
  HTMLMediaElement.prototype.load = function () {};
}
