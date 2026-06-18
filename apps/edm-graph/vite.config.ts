import { defineConfig } from "vite";

// Minimal config: project root is the directory, and the default `public/`
// directory is served statically so `/graph.json` is fetchable at runtime.
export default defineConfig({
  server: {
    // Honor a PORT assigned by the environment (e.g. the preview harness),
    // falling back to Vite's default when unset.
    port: process.env.PORT ? Number(process.env.PORT) : undefined
  }
});
