import { defineConfig } from "astro/config";
import tailwind from "@astrojs/tailwind";

export default defineConfig({
  site: "https://sjroesink.github.io",
  base: "/Buddio",
  integrations: [tailwind({ applyBaseStyles: false })],
});
