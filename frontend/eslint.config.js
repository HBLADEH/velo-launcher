import js from '@eslint/js'
import ts from 'typescript-eslint'
import vue from 'eslint-plugin-vue'

export default ts.config(
  js.configs.recommended,
  ...ts.configs.recommended,
  ...vue.configs['flat/essential'],
  { files: ['**/*.vue'], languageOptions: { parserOptions: { parser: ts.parser } } },
  // TypeScript checks undefined identifiers, including DOM types in Vue scripts.
  { files: ['src/**/*.{ts,vue}'], rules: { 'no-undef': 'off' } },
)
