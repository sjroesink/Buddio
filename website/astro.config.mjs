import { defineConfig } from "astro/config";
import tailwind from "@astrojs/tailwind";

export default defineConfig({
  site: "https://buddio.roesink.dev",
  integrations: [tailwind({ applyBaseStyles: false })],
});
