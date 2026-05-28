import next from "eslint-config-next";
import coreWebVitals from "eslint-config-next/core-web-vitals";

const config = [
  ...next,
  ...coreWebVitals,
  {
    rules: {
      "@next/next/no-img-element": "off",
    },
  },
];

export default config;
