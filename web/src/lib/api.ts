export function getGraphQLEndpoint() {
  if (import.meta.env.MODE === "development") {
    return "http://localhost:8000/";
  }
  if (import.meta.env.PROD && !import.meta.env.VITE_API_URL) {
    throw new Error("VITE_API_URL must be set for a production build");
  }
  return import.meta.env.VITE_API_URL ?? "http://localhost:8000/";
}
