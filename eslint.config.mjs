// ESLint flat config (ESLint 9). Replaces the old .eslintrc.cjs / .eslintignore with the same
// rule set: eslint:recommended, React, typescript-eslint recommended and Prettier.
import js from '@eslint/js'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import react from 'eslint-plugin-react'
import prettierRecommended from 'eslint-plugin-prettier/recommended'

export default tseslint.config(
  {
    ignores: [
      '**/node_modules/**',
      'dist/**',
      'out/**',
      'src-tauri/target/**',
      'src-tauri/app/gen/**',
      'notices/**',
      '.flowbite-react/**'
    ]
  },
  js.configs.recommended,
  react.configs.flat.recommended,
  react.configs.flat['jsx-runtime'],
  tseslint.configs.recommended,
  prettierRecommended,
  {
    files: ['**/*.{js,jsx,cjs,mjs,ts,tsx,cts,mts}'],
    languageOptions: {
      ecmaVersion: 2021,
      sourceType: 'module',
      parserOptions: { ecmaFeatures: { jsx: true } },
      globals: { ...globals.browser, ...globals.node, ...globals.commonjs, ...globals.es2021 }
    },
    linterOptions: {
      // ESLint 8 didn't report unused eslint-disable comments by default; keep it that way.
      reportUnusedDisableDirectives: 'off'
    },
    settings: {
      react: { version: 'detect' }
    },
    rules: {
      // TypeScript rules this project has always used (formerly from @electron-toolkit/eslint-config-ts)
      '@typescript-eslint/ban-ts-comment': ['error', { 'ts-ignore': 'allow-with-description' }],
      '@typescript-eslint/explicit-module-boundary-types': 'off',
      '@typescript-eslint/no-empty-function': ['error', { allow: ['arrowFunctions'] }],
      '@typescript-eslint/no-empty-object-type': [
        'error',
        { allowInterfaces: 'with-single-extends' }
      ],
      '@typescript-eslint/no-non-null-assertion': 'off',
      '@typescript-eslint/no-require-imports': 'off',
      'prettier/prettier': 'warn',

      '@typescript-eslint/explicit-function-return-type': 'off',
      '@typescript-eslint/no-explicit-any': 'off',
      '@typescript-eslint/no-unused-vars': [
        'error',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', caughtErrors: 'none' }
      ],
      'react/prop-types': 'off',
      'no-control-regex': 0,
      'no-restricted-syntax': [
        'warn',
        {
          selector: 'ImportExpression',
          message:
            'Consider using static imports instead of dynamic imports unless specifically required.'
        }
      ]
    }
  }
)
