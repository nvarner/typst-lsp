module.exports = {
    env: {
        es2021: true,
        node: true,
    },
    extends: ["airbnb-base", "airbnb-typescript/base", "prettier"],
    overrides: [],
    parserOptions: {
        ecmaVersion: "latest",
        sourceType: "module",
        project: "tsconfig.json",
    },
    rules: {
        "no-unused-vars": "off",
        "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
    },
};
