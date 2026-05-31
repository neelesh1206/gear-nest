import type { NextConfig } from "next";

const config: NextConfig = {
  reactStrictMode: true,
  images: {
    remotePatterns: [
      { protocol: "https", hostname: "images.unsplash.com" },
      { protocol: "https", hostname: "cdn.gearnest.io" },
      { protocol: "https", hostname: "placehold.co" },
    ],
  },
};

export default config;
